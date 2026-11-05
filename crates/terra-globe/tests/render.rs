//! Deterministic render snapshots.
//!
//! The tile provider is replaced by synthetic checkerboards so the image is
//! independent of the network and of any real imagery. Run with
//! `UPDATE_SNAPSHOTS=1` to regenerate the baseline after an intended change.

use std::path::Path;

use image::RgbaImage;
use terra_globe::geo::{Geodetic, TileId};
use terra_globe::imagery::{FetchedTile, TILE_TEXTURE_SIZE, TileProvider};
use terra_globe::{Camera, Globe};

fn headless_device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .ok()?;
    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).ok()
}

fn checkerboard(seed: u32) -> RgbaImage {
    let mut image = RgbaImage::new(TILE_TEXTURE_SIZE, TILE_TEXTURE_SIZE);
    let cell = 16;
    for (x, y, pixel) in image.enumerate_pixels_mut() {
        let checker = ((x / cell) + (y / cell) + seed).is_multiple_of(2);
        *pixel = if checker {
            image::Rgba([220, 210, 190, 255])
        } else {
            image::Rgba([40, 90, 140, 255])
        };
    }
    image
}

fn fixed_camera() -> Camera {
    let mut camera = Camera::default();
    camera.target_lon_deg = 10.0;
    camera.target_lat_deg = 35.0;
    camera.distance = 18_000_000.0;
    camera.heading = 0.0;
    camera.pitch = 42f64.to_radians();
    camera
}

/// A provider that can never answer, so snapshots stay independent of the
/// network and of when a fetch happens to finish.
fn offline_provider() -> TileProvider {
    let mut source = terra_providers::TileSource::open_street_map();
    source.url = "http://127.0.0.1:1/{z}/{x}/{y}.png".to_string();
    TileProvider::new(source)
}

fn snapshot_globe() -> Option<Globe> {
    let (device, queue) = headless_device()?;
    let mut globe = Globe::new(device, queue, offline_provider(), (800, 500));
    globe.camera = fixed_camera();

    // A few levels of real textures, using the same ancestor/UV fallback path
    // the live renderer uses.
    let mut tiles = vec![TileId::root()];
    for child in TileId::root().children().into_iter().flatten() {
        tiles.push(child);
    }
    let fetched: Vec<FetchedTile> = tiles
        .into_iter()
        .map(|id| FetchedTile {
            id,
            result: Ok(checkerboard(u32::from(id.z) + id.x + id.y)),
        })
        .collect();
    globe.renderer.ingest(fetched);
    Some(globe)
}

#[test]
fn globe_render_matches_the_snapshot() {
    let Some(mut globe) = snapshot_globe() else {
        eprintln!("no GPU adapter available; skipping globe snapshot");
        return;
    };

    let stats = globe.render(0.0);
    assert!(stats.tiles_drawn > 0, "renderer drew no tiles");
    assert!(stats.textures_ready >= 5);

    let (width, height, pixels) = globe
        .renderer
        .read_pixels()
        .expect("readback must succeed on a working adapter");
    let png = terra_testkit::rgba_to_png(&pixels, width, height);
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots/globe.png");
    terra_testkit::check_or_update_snapshot(&path, &png, 2, 200)
        .unwrap_or_else(|mismatch| panic!("globe render drifted: {mismatch}"));
}

#[test]
fn rendering_is_deterministic_across_frames() {
    let Some(mut globe) = snapshot_globe() else {
        eprintln!("no GPU adapter available; skipping determinism check");
        return;
    };
    globe.render(0.0);
    let (_, _, first) = globe.renderer.read_pixels().expect("first readback");
    globe.render(0.0);
    let (_, _, second) = globe.renderer.read_pixels().expect("second readback");
    let differing = first
        .iter()
        .zip(second.iter())
        .filter(|(left, right)| left != right)
        .count();
    assert_eq!(
        differing, 0,
        "identical frames must produce identical pixels ({differing} bytes differ)"
    );
}

#[test]
fn marker_projection_tracks_the_globe_point() {
    let Some(globe) = snapshot_globe() else {
        return;
    };
    // The camera target must be at the centre of the viewport.
    let target = globe
        .project(Geodetic::from_degrees(10.0, 35.0, 0.0))
        .expect("target is visible");
    assert!((target.x - 400.0).abs() < 1.0);
    assert!((target.y - 250.0).abs() < 1.0);

    // A point on the far side must not be projected at all.
    assert!(
        globe
            .project(Geodetic::from_degrees(-170.0, -35.0, 0.0))
            .is_none()
    );
}

#[test]
fn zooming_in_selects_finer_tiles() {
    let Some(mut globe) = snapshot_globe() else {
        return;
    };
    let far = globe.render(0.0);
    globe.camera.distance = 3_000.0;
    globe.camera.pitch = 70f64.to_radians();
    let near = globe.render(0.0);
    assert!(
        near.deepest_level > far.deepest_level,
        "far {:?} near {:?}",
        far,
        near
    );
}

#[test]
fn polar_caps_cover_the_gap_beyond_web_mercator() {
    let Some(mut globe) = snapshot_globe() else {
        eprintln!("no GPU adapter available; skipping polar cap check");
        return;
    };
    globe.camera.target_lat_deg = 89.9;
    globe.camera.distance = 1_500_000.0;
    globe.camera.pitch = 75f64.to_radians();
    globe.render(0.0);

    let (width, height, pixels) = globe.renderer.read_pixels().expect("readback");
    let centre = ((height / 2) * width + width / 2) as usize * 4;
    let pixel = [pixels[centre], pixels[centre + 1], pixels[centre + 2]];
    // The clear colour is the space background; a pole with no geometry would
    // show it at the centre of the viewport.
    assert!(
        pixel != [2, 3, 5],
        "pole is not covered by any surface (pixel {pixel:?})"
    );

    let png = terra_testkit::rgba_to_png(&pixels, width, height);
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots/globe-pole.png");
    terra_testkit::check_or_update_snapshot(&path, &png, 2, 200)
        .unwrap_or_else(|mismatch| panic!("polar render drifted: {mismatch}"));
}

#[test]
fn scene_geometry_is_drawn_over_the_globe() {
    let Some(mut globe) = snapshot_globe() else {
        eprintln!("no GPU adapter available; skipping scene render");
        return;
    };
    globe.camera.distance = 900_000.0;
    globe.camera.pitch = 60f64.to_radians();

    globe.render(0.0);
    let (_, _, without_scene) = globe.renderer.read_pixels().expect("readback");

    let center = terra_core::GeoPoint::new(10.0, 35.0).expect("point");
    let style = terra_core::Style {
        outline_enabled: true,
        outline_color: "#ffffff".into(),
        outline_width: 2.0,
        filled: true,
        fill_color: "#ff5533".into(),
        fill_opacity: 0.55,
        ..terra_core::Style::default()
    };
    let circle = terra_globe::SceneEntry {
        id: "circle".into(),
        geometry: terra_core::PlacemarkGeometry::circle(center, 80_000.0).expect("circle"),
        style: style.clone(),
        visible: true,
        selected: false,
    };
    let line = terra_globe::SceneEntry {
        id: "line".into(),
        geometry: terra_core::PlacemarkGeometry::line_string(
            vec![
                terra_core::GeoPoint::new(10.5, 35.2).expect("point"),
                terra_core::GeoPoint::new(11.2, 35.5).expect("point"),
            ],
            None,
            None,
        )
        .expect("line"),
        style: style.clone(),
        visible: true,
        selected: false,
    };
    globe.set_scene(
        1,
        terra_globe::Scene {
            entries: vec![circle, line],
            ..Default::default()
        },
    );

    let stats = globe.render(0.0);
    assert_eq!(stats.scene_items, 2, "both entries produced meshes");
    let (width, height, with_scene) = globe.renderer.read_pixels().expect("readback");
    assert_ne!(
        without_scene, with_scene,
        "scene geometry must change the frame"
    );

    let png = terra_testkit::rgba_to_png(&with_scene, width, height);
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots/globe-scene.png");
    terra_testkit::check_or_update_snapshot(&path, &png, 3, 600)
        .unwrap_or_else(|mismatch| panic!("scene render drifted: {mismatch}"));
}

#[test]
fn flat_modes_render_and_pick() {
    let Some(mut globe) = snapshot_globe() else {
        eprintln!("no GPU adapter available; skipping flat modes");
        return;
    };
    let target = terra_core::GeoPoint::new(10.0, 35.0).expect("point");
    globe.set_scene(
        1,
        terra_globe::Scene {
            entries: vec![terra_globe::SceneEntry {
                id: "circle".into(),
                geometry: terra_core::PlacemarkGeometry::circle(target, 120_000.0).expect("circle"),
                style: terra_core::Style {
                    outline_enabled: true,
                    outline_color: "#ffffff".into(),
                    outline_width: 2.0,
                    filled: true,
                    fill_color: "#ff5533".into(),
                    fill_opacity: 0.6,
                    ..terra_core::Style::default()
                },
                visible: true,
                selected: false,
            }],
            ..Default::default()
        },
    );
    globe.flat.fly_to(&target, 2_000_000.0);

    globe.set_mode(terra_globe::SceneMode::Scene2D);
    globe.render(0.0);
    let (width, height, pixels) = globe.renderer.read_pixels().expect("readback");
    let lit = pixels
        .chunks(4)
        .filter(|pixel| pixel[0] > 12 || pixel[1] > 12 || pixel[2] > 12)
        .count();
    assert!(lit > 1000, "2D render is blank ({lit} lit pixels)");
    let png = terra_testkit::rgba_to_png(&pixels, width, height);
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots/globe-2d.png");
    terra_testkit::check_or_update_snapshot(&path, &png, 3, 600)
        .unwrap_or_else(|mismatch| panic!("2D render drifted: {mismatch}"));

    let picked = globe
        .pick_placemark((400.0, 250.0))
        .map(|hit| hit.id)
        .or_else(|| globe.pick((400.0, 250.0)).map(|_| "surface".to_string()));
    assert_eq!(
        picked.as_deref(),
        Some("circle"),
        "centre of the 2D view hits the circle"
    );
    let surface = globe.pick((400.0, 250.0)).expect("plane pick");
    assert!((surface.lon_deg - 10.0).abs() < 0.5 && (surface.lat_deg - 35.0).abs() < 0.5);

    globe.set_mode(terra_globe::SceneMode::Columbus);
    globe.render(0.0);
    let (width, height, pixels) = globe.renderer.read_pixels().expect("readback");
    let png = terra_testkit::rgba_to_png(&pixels, width, height);
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/snapshots/globe-columbus.png");
    terra_testkit::check_or_update_snapshot(&path, &png, 3, 600)
        .unwrap_or_else(|mismatch| panic!("Columbus render drifted: {mismatch}"));

    globe.set_mode(terra_globe::SceneMode::Scene3D);
    assert_eq!(globe.mode(), terra_globe::SceneMode::Scene3D);
}
