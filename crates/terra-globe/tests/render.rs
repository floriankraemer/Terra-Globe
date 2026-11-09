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
            result: Ok(checkerboard(u32::from(id.z) + id.x + id.y).into()),
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

fn red_fill_style() -> terra_core::Style {
    terra_core::Style {
        outline_enabled: false,
        outline_color: "#ff2200".into(),
        outline_width: 1.0,
        filled: true,
        fill_color: "#ff2200".into(),
        fill_opacity: 1.0,
        ..terra_core::Style::default()
    }
}

/// Renders `geometry` at each camera distance and returns a description of
/// every sample point (a grid of `lon/lat` offsets around the target) that
/// shows something other than the fill colour.
fn hollow_samples(
    name: &str,
    geometry: terra_core::PlacemarkGeometry,
    target: (f64, f64),
    half_extent_deg: (f64, f64),
    distances_km: &[f64],
) -> Vec<String> {
    let Some(mut globe) = snapshot_globe() else {
        eprintln!("no GPU adapter available; skipping {name}");
        return Vec::new();
    };
    globe.camera.target_lon_deg = target.0;
    globe.camera.target_lat_deg = target.1;
    globe.camera.pitch = 60f64.to_radians();
    globe.set_scene(
        1,
        terra_globe::Scene {
            entries: vec![terra_globe::SceneEntry {
                id: name.into(),
                geometry,
                style: red_fill_style(),
                visible: true,
                selected: false,
            }],
            ..Default::default()
        },
    );

    let mut failures = Vec::new();
    for &distance_km in distances_km {
        globe.camera.distance = distance_km * 1000.0;
        globe.render(0.0);
        let (width, height, pixels) = globe.renderer.read_pixels().expect("readback");
        for row in -2i32..=2 {
            for column in -2i32..=2 {
                let lon = target.0 + f64::from(column) / 2.0 * half_extent_deg.0;
                let lat = target.1 + f64::from(row) / 2.0 * half_extent_deg.1;
                let on_screen = globe
                    .project(Geodetic::from_degrees(lon, lat, 0.0))
                    .filter(|p| {
                        p.x >= 0.0 && p.y >= 0.0 && p.x < width as f64 && p.y < height as f64
                    });
                let Some(screen) = on_screen else {
                    assert!(
                        (row, column) != (0, 0),
                        "{name} @{distance_km} km: the target is not on screen"
                    );
                    continue;
                };
                let index = (screen.y as usize * width as usize + screen.x as usize) * 4;
                let pixel = [pixels[index], pixels[index + 1], pixels[index + 2]];
                if !(pixel[0] > 180 && pixel[1] < 120 && pixel[2] < 120) {
                    failures.push(format!(
                        "{name} @{distance_km} km: ({lon:.1},{lat:.1}) shows {pixel:?}"
                    ));
                }
            }
        }
    }
    failures
}

const FILL_DISTANCES_KM: [f64; 4] = [600.0, 2_000.0, 6_000.0, 20_000.0];

#[test]
fn large_filled_shapes_are_not_hollowed_out_by_the_globe() {
    let center = terra_core::GeoPoint::new(10.0, 35.0).expect("point");
    let mut failures = Vec::new();

    // Sample offsets stay at ~60% of the radius so every point is inside.
    for (name, radius_m) in [("circle-150km", 150_000.0), ("circle-2000km", 2_000_000.0)] {
        let half_lat = radius_m * 0.6 / 111_000.0;
        let half_lon = half_lat / 35f64.to_radians().cos();
        failures.extend(hollow_samples(
            name,
            terra_core::PlacemarkGeometry::circle(center, radius_m).expect("circle"),
            (10.0, 35.0),
            (half_lon * 0.7, half_lat * 0.7),
            &FILL_DISTANCES_KM,
        ));
    }

    // Large enough to hit the triangle cap with 5 km edges.
    let corner = |lon, lat| terra_core::GeoPoint::new(lon, lat).expect("point");
    failures.extend(hollow_samples(
        "continent",
        terra_core::PlacemarkGeometry::polygon(
            vec![
                corner(-10.0, 38.0),
                corner(35.0, 38.0),
                corner(35.0, 62.0),
                corner(-10.0, 62.0),
                corner(-10.0, 38.0),
            ],
            None,
            None,
            None,
        )
        .expect("polygon"),
        (12.5, 50.0),
        (18.0, 8.0),
        &FILL_DISTANCES_KM,
    ));

    assert!(
        failures.is_empty(),
        "fill hollowed out by the globe:\n{}",
        failures.join("\n")
    );
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

#[test]
fn tiles_that_arrive_late_in_a_session_are_drawn() {
    let Some(mut globe) = snapshot_globe() else {
        eprintln!("no GPU adapter available; skipping late-arrival check");
        return;
    };

    // Run long enough that the texture eviction window (240 frames) has passed.
    for frame in 0..260 {
        globe.render(f64::from(frame) / 60.0);
    }

    // A burst of tiles arriving at once, as a cache hit or a fast server
    // delivers them, pushes the texture count over the eviction threshold. The
    // fresh tiles, including some the current view needs, must survive it.
    let fetched: Vec<FetchedTile> = (0..400u32)
        .filter_map(|index| TileId::new(9, index, 500))
        .chain(TileId::new(3, 4, 3))
        .chain(TileId::new(3, 4, 2))
        .map(|id| FetchedTile {
            id,
            result: Ok(checkerboard(id.x).into()),
        })
        .collect();
    globe.renderer.ingest(fetched);

    for _ in 0..3 {
        let stats = globe.render(5.0);
        assert_eq!(
            stats.tiles_drawn,
            globe.renderer.last_selection().len(),
            "selected tiles went undrawn, leaving holes in the globe"
        );
    }
}

mod overlays {
    use std::sync::Arc;

    use super::*;
    use terra_globe::{ImageOverlay, SceneMode, TileOverlay};
    use terra_providers::TileSource;

    const GREEN: [u8; 4] = [40, 160, 60, 255];
    const RED: [u8; 4] = [255, 0, 0, 255];
    const BLUE: [u8; 4] = [0, 0, 255, 255];
    const FALLBACK: [u8; 4] = [58, 74, 88, 255];
    const TARGET: (f64, f64) = (10.0, 35.0);

    fn solid(width: u32, height: u32, color: [u8; 4]) -> RgbaImage {
        RgbaImage::from_pixel(width, height, image::Rgba(color))
    }

    fn layer_source(id: &str) -> TileSource {
        let mut source = TileSource::open_street_map();
        source.id = id.to_string();
        source.name = id.to_string();
        source.credit = format!("credit of {id}");
        source.url = "http://127.0.0.1:1/{z}/{x}/{y}.png".to_string();
        source
    }

    fn layer(id: &str, opacity: f32) -> TileOverlay {
        TileOverlay {
            source: layer_source(id),
            opacity,
        }
    }

    /// A top-down view over a uniformly green basemap, so every pixel's
    /// expected colour is a plain blend.
    fn overlay_globe() -> Option<Globe> {
        let (device, queue) = headless_device()?;
        let mut globe = Globe::new(device, queue, offline_provider(), (800, 500));
        globe.camera.target_lon_deg = TARGET.0;
        globe.camera.target_lat_deg = TARGET.1;
        globe.camera.distance = 2_000_000.0;
        globe.camera.heading = 0.0;
        globe.camera.pitch = std::f64::consts::FRAC_PI_2;
        globe.renderer.ingest(vec![FetchedTile {
            id: TileId::root(),
            result: Ok(solid(TILE_TEXTURE_SIZE, TILE_TEXTURE_SIZE, GREEN).into()),
        }]);
        Some(globe)
    }

    fn ingest_layer(globe: &mut Globe, id: &str, color: [u8; 4]) {
        globe.renderer.ingest_overlay(
            id,
            vec![FetchedTile {
                id: TileId::root(),
                result: Ok(solid(TILE_TEXTURE_SIZE, TILE_TEXTURE_SIZE, color).into()),
            }],
        );
    }

    fn frame(globe: &mut Globe) -> (u32, Vec<u8>) {
        globe.render(0.0);
        let (width, _, pixels) = globe.renderer.read_pixels().expect("readback");
        (width, pixels)
    }

    fn rgb_at(width: u32, pixels: &[u8], x: f64, y: f64) -> [u8; 3] {
        let index = (y as usize * width as usize + x as usize) * 4;
        [pixels[index], pixels[index + 1], pixels[index + 2]]
    }

    fn centre(globe: &mut Globe) -> [u8; 3] {
        let (width, pixels) = frame(globe);
        rgb_at(width, &pixels, 400.0, 250.0)
    }

    fn at(globe: &mut Globe, lon: f64, lat: f64) -> [u8; 3] {
        let screen = globe
            .project(Geodetic::from_degrees(lon, lat, 0.0))
            .unwrap_or_else(|| panic!("({lon}, {lat}) must be visible"));
        let (width, pixels) = frame(globe);
        rgb_at(width, &pixels, screen.x, screen.y)
    }

    #[track_caller]
    fn assert_close(actual: [u8; 3], expected: [u8; 4], tolerance: i32) {
        for channel in 0..3 {
            let delta = (i32::from(actual[channel]) - i32::from(expected[channel])).abs();
            assert!(
                delta <= tolerance,
                "expected ~{expected:?}, got {actual:?} (channel {channel})"
            );
        }
    }

    fn mix(top: [u8; 4], bottom: [u8; 4], opacity: f32) -> [u8; 4] {
        let blend = |t: u8, b: u8| (f32::from(t) * opacity + f32::from(b) * (1.0 - opacity)) as u8;
        [
            blend(top[0], bottom[0]),
            blend(top[1], bottom[1]),
            blend(top[2], bottom[2]),
            255,
        ]
    }

    fn image_overlay(id: &str, bounds: (f64, f64, f64, f64), image: RgbaImage) -> ImageOverlay {
        ImageOverlay {
            id: id.to_string(),
            west: bounds.0,
            south: bounds.1,
            east: bounds.2,
            north: bounds.3,
            rotation_deg: 0.0,
            opacity: 1.0,
            image: Arc::new(image),
        }
    }

    fn near_target(image: RgbaImage) -> ImageOverlay {
        image_overlay("photo", (9.0, 34.0, 11.0, 36.0), image)
    }

    #[test]
    fn half_opaque_tile_overlay_mixes_with_the_basemap() {
        let Some(mut globe) = overlay_globe() else {
            eprintln!("no GPU adapter available; skipping overlay blend");
            return;
        };
        assert_close(centre(&mut globe), GREEN, 2);
        globe.set_tile_overlays(vec![layer("red", 0.5)]);
        ingest_layer(&mut globe, "red", RED);
        assert_close(centre(&mut globe), mix(RED, GREEN, 0.5), 3);
    }

    #[test]
    fn opaque_tile_overlay_covers_the_basemap() {
        let Some(mut globe) = overlay_globe() else {
            return;
        };
        globe.set_tile_overlays(vec![layer("red", 1.0)]);
        ingest_layer(&mut globe, "red", RED);
        assert_close(centre(&mut globe), RED, 2);
    }

    #[test]
    fn transparent_overlay_pixels_leave_the_basemap_untouched() {
        let Some(mut globe) = overlay_globe() else {
            return;
        };
        let (_, base) = frame(&mut globe);
        globe.set_tile_overlays(vec![layer("clear", 1.0)]);
        ingest_layer(&mut globe, "clear", [255, 0, 0, 0]);
        let (_, with_layer) = frame(&mut globe);
        assert_eq!(base, with_layer, "alpha 0 must not change a single byte");
    }

    #[test]
    fn partly_transparent_overlay_pixels_use_their_alpha() {
        let Some(mut globe) = overlay_globe() else {
            return;
        };
        globe.set_tile_overlays(vec![layer("soft", 1.0)]);
        ingest_layer(&mut globe, "soft", [255, 0, 0, 128]);
        assert_close(centre(&mut globe), mix(RED, GREEN, 128.0 / 255.0), 4);
    }

    #[test]
    fn stacked_tile_overlays_blend_bottom_to_top() {
        let Some(mut globe) = overlay_globe() else {
            return;
        };
        globe.set_tile_overlays(vec![layer("red", 1.0), layer("blue", 0.5)]);
        ingest_layer(&mut globe, "red", RED);
        ingest_layer(&mut globe, "blue", BLUE);
        assert_close(centre(&mut globe), mix(BLUE, RED, 0.5), 3);

        globe.set_tile_overlays(vec![layer("blue", 0.5), layer("red", 1.0)]);
        assert_close(centre(&mut globe), RED, 2);
    }

    #[test]
    fn zero_opacity_overlays_change_nothing() {
        let Some(mut globe) = overlay_globe() else {
            return;
        };
        let (_, base) = frame(&mut globe);
        globe.set_tile_overlays(vec![layer("red", 0.0)]);
        ingest_layer(&mut globe, "red", RED);
        globe.set_image_overlays(vec![ImageOverlay {
            opacity: 0.0,
            ..near_target(solid(8, 8, BLUE))
        }]);
        let (_, hidden) = frame(&mut globe);
        assert_eq!(base, hidden);
    }

    #[test]
    fn removing_overlays_restores_the_basemap() {
        let Some(mut globe) = overlay_globe() else {
            return;
        };
        let (_, base) = frame(&mut globe);
        globe.set_tile_overlays(vec![layer("red", 1.0)]);
        ingest_layer(&mut globe, "red", RED);
        globe.set_image_overlays(vec![near_target(solid(8, 8, BLUE))]);
        assert_close(centre(&mut globe), BLUE, 2);

        globe.set_tile_overlays(Vec::new());
        globe.set_image_overlays(Vec::new());
        let (_, restored) = frame(&mut globe);
        assert_eq!(base, restored);
    }

    #[test]
    fn overlay_tiles_that_fail_to_load_are_simply_not_drawn() {
        let Some(mut globe) = overlay_globe() else {
            return;
        };
        let (_, base) = frame(&mut globe);
        // The URL points at a closed port, so every request fails for real.
        globe.set_tile_overlays(vec![layer("dead", 1.0)]);
        globe.renderer.ingest_overlay(
            "dead",
            vec![FetchedTile {
                id: TileId::root(),
                result: Err("HTTP 500".to_string()),
            }],
        );
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while std::time::Instant::now() < deadline {
            globe.render(0.0);
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let (_, after) = frame(&mut globe);
        assert_eq!(base, after);
        assert!(!globe.renderer.has_overlay_texture("dead", TileId::root()));
    }

    #[test]
    fn overlay_tiles_without_data_do_not_break_the_basemap() {
        let Some(mut globe) = overlay_globe() else {
            return;
        };
        let server = tiny_http::Server::http("127.0.0.1:0").expect("bind");
        let port = server.server_addr().to_ip().expect("tcp").port();
        std::thread::spawn(move || {
            for request in server.incoming_requests() {
                let _ = request.respond(tiny_http::Response::empty(404));
            }
        });
        let mut source = layer_source("missing");
        source.url = format!("http://127.0.0.1:{port}/{{z}}/{{x}}/{{y}}.png");
        let (_, base) = frame(&mut globe);
        globe.set_tile_overlays(vec![TileOverlay {
            source,
            opacity: 1.0,
        }]);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let mut asked = false;
        while std::time::Instant::now() < deadline && !(asked && !globe.is_loading_imagery()) {
            globe.render(0.0);
            asked |= globe.is_loading_imagery();
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(asked, "the layer never requested a tile");
        let (_, after) = frame(&mut globe);
        assert_eq!(base, after);
    }

    #[test]
    fn switching_the_basemap_keeps_the_overlays() {
        let Some(mut globe) = overlay_globe() else {
            return;
        };
        globe.set_tile_overlays(vec![layer("red", 0.5)]);
        ingest_layer(&mut globe, "red", RED);
        globe.set_image_overlays(vec![near_target(solid(8, 8, [0, 0, 255, 0]))]);

        let mut other = offline_provider().source;
        other.id = "other-basemap".to_string();
        globe.set_provider(TileProvider::new(other));
        assert!(globe.renderer.has_overlay_texture("red", TileId::root()));
        assert_eq!(globe.renderer.image_overlay_count(), 1);
        assert_close(centre(&mut globe), mix(RED, FALLBACK, 0.5), 3);
    }

    #[test]
    fn image_overlay_is_visible_inside_its_bounds_and_not_outside() {
        let Some(mut globe) = overlay_globe() else {
            return;
        };
        globe.set_image_overlays(vec![near_target(solid(32, 32, BLUE))]);
        assert_close(at(&mut globe, 10.0, 35.0), BLUE, 2);
        assert_close(at(&mut globe, 9.3, 34.3), BLUE, 2);
        assert_close(at(&mut globe, 12.0, 35.0), GREEN, 2);
        assert_close(at(&mut globe, 10.0, 37.0), GREEN, 2);
    }

    #[test]
    fn overlays_are_not_hollowed_out_by_the_globe_at_any_distance() {
        let Some(mut globe) = overlay_globe() else {
            return;
        };
        globe.set_tile_overlays(vec![layer("red", 1.0)]);
        ingest_layer(&mut globe, "red", RED);
        globe.set_image_overlays(vec![image_overlay(
            "big",
            (7.0, 33.0, 13.0, 37.0),
            solid(32, 32, BLUE),
        )]);
        globe.camera.pitch = 60f64.to_radians();
        let mut failures = Vec::new();
        for distance_km in [0.5, 5.0, 60.0, 600.0, 6_000.0, 20_000.0] {
            globe.camera.distance = distance_km * 1000.0;
            let (width, pixels) = frame(&mut globe);
            for row in -2i32..=2 {
                for column in -2i32..=2 {
                    let lon = TARGET.0 + f64::from(column) * 0.4;
                    let lat = TARGET.1 + f64::from(row) * 0.3;
                    let Some(screen) =
                        globe
                            .project(Geodetic::from_degrees(lon, lat, 0.0))
                            .filter(|p| {
                                p.x >= 0.0 && p.y >= 0.0 && p.x < f64::from(width) && p.y < 500.0
                            })
                    else {
                        continue;
                    };
                    let pixel = rgb_at(width, &pixels, screen.x, screen.y);
                    if pixel[2] < 240 || pixel[0] > 15 {
                        failures.push(format!("@{distance_km} km ({lon:.1},{lat:.1}) {pixel:?}"));
                    }
                }
            }
        }
        assert!(
            failures.is_empty(),
            "image lost the depth test:\n{}",
            failures.join("\n")
        );
    }

    #[test]
    fn continent_sized_images_are_not_hollowed_out_by_the_base_tiles() {
        let Some(mut globe) = overlay_globe() else {
            return;
        };
        globe.camera.target_lon_deg = 0.0;
        globe.camera.target_lat_deg = 0.0;
        globe.camera.pitch = 60f64.to_radians();
        let cases = [
            ("continent", (-60.0, -50.0, 60.0, 50.0), 0.0, 40.0, 30.0),
            ("hemisphere", (-85.0, -40.0, 85.0, 40.0), 0.0, 60.0, 25.0),
            ("rotated", (-60.0, -50.0, 60.0, 50.0), 30.0, 25.0, 20.0),
        ];
        let mut failures = Vec::new();
        for (name, bounds, rotation_deg, half_lon, half_lat) in cases {
            globe.set_image_overlays(vec![ImageOverlay {
                rotation_deg,
                ..image_overlay(name, bounds, solid(32, 32, BLUE))
            }]);
            for distance_km in [20.0, 100.0, 600.0, 2_000.0, 6_000.0, 20_000.0] {
                globe.camera.distance = distance_km * 1000.0;
                let (width, pixels) = frame(&mut globe);
                // Close up, only the middle of a continent is on screen.
                let reach_deg = distance_km / 111.0 * 0.5;
                for row in -4i32..=4 {
                    for column in -4i32..=4 {
                        let lon = (f64::from(column) + 0.37) / 4.0 * f64::min(half_lon, reach_deg);
                        let lat = (f64::from(row) + 0.21) / 4.0 * f64::min(half_lat, reach_deg);
                        let Some(screen) = globe
                            .project(Geodetic::from_degrees(lon, lat, 0.0))
                            .filter(|p| {
                                p.x >= 0.0 && p.y >= 0.0 && p.x < f64::from(width) && p.y < 500.0
                            })
                        else {
                            continue;
                        };
                        let pixel = rgb_at(width, &pixels, screen.x, screen.y);
                        if pixel[1] > 15 {
                            failures.push(format!(
                                "{name} @{distance_km} km ({lon:.0},{lat:.0}) {pixel:?}"
                            ));
                        }
                    }
                }
            }
        }
        assert!(
            failures.is_empty(),
            "globe leaked through the image:\n{}",
            failures.join("\n")
        );
    }

    #[test]
    fn overlays_work_in_columbus_view() {
        let Some(mut globe) = overlay_globe() else {
            return;
        };
        globe.flat.fly_to(
            &terra_core::GeoPoint::new(TARGET.0, TARGET.1).expect("point"),
            2_000_000.0,
        );
        globe.set_mode(SceneMode::Columbus);
        globe.set_tile_overlays(vec![layer("red", 0.5)]);
        ingest_layer(&mut globe, "red", RED);
        assert_close(at(&mut globe, 10.0, 35.0), mix(RED, GREEN, 0.5), 3);

        globe.set_image_overlays(vec![near_target(solid(16, 16, BLUE))]);
        assert_close(at(&mut globe, 10.0, 35.0), BLUE, 2);
        assert_close(at(&mut globe, 10.0, 38.0), mix(RED, GREEN, 0.5), 3);
    }

    #[test]
    fn image_overlay_opacity_mixes_with_what_is_below() {
        let Some(mut globe) = overlay_globe() else {
            return;
        };
        globe.set_image_overlays(vec![ImageOverlay {
            opacity: 0.25,
            ..near_target(solid(32, 32, BLUE))
        }]);
        assert_close(centre(&mut globe), mix(BLUE, GREEN, 0.25), 3);
    }

    #[test]
    fn image_overlay_keeps_its_orientation() {
        let Some(mut globe) = overlay_globe() else {
            return;
        };
        let mut picture = solid(64, 64, BLUE);
        for (x, _, pixel) in picture.enumerate_pixels_mut() {
            if x < 32 {
                *pixel = image::Rgba(RED);
            }
        }
        globe.set_image_overlays(vec![near_target(picture)]);
        assert_close(at(&mut globe, 9.5, 35.0), RED, 3);
        assert_close(at(&mut globe, 10.5, 35.0), BLUE, 3);
    }

    #[test]
    fn rotating_a_picture_turns_it_counter_clockwise() {
        let Some(mut globe) = overlay_globe() else {
            return;
        };
        globe.camera.target_lat_deg = 0.0;
        globe.camera.target_lon_deg = 0.0;
        let mut picture = solid(64, 64, BLUE);
        for (x, _, pixel) in picture.enumerate_pixels_mut() {
            if x < 32 {
                *pixel = image::Rgba(RED);
            }
        }
        let mut overlay = image_overlay("square", (-1.0, -1.0, 1.0, 1.0), picture);

        overlay.rotation_deg = 90.0;
        globe.set_image_overlays(vec![overlay.clone()]);
        // The red west half turned to the south, the blue east half to the north.
        assert_close(at(&mut globe, 0.0, -0.5), RED, 3);
        assert_close(at(&mut globe, 0.0, 0.5), BLUE, 3);

        overlay.rotation_deg = 45.0;
        overlay.image = Arc::new(solid(32, 32, BLUE));
        globe.set_image_overlays(vec![overlay]);
        // A diamond: the tip pokes out past the old east edge, the old corner is empty.
        assert_close(at(&mut globe, 1.3, 0.0), BLUE, 2);
        assert_close(at(&mut globe, 0.95, 0.95), GREEN, 2);
        assert_close(at(&mut globe, 0.0, 0.0), BLUE, 2);
    }

    #[test]
    fn image_overlay_across_the_antimeridian_is_one_piece() {
        let Some(mut globe) = overlay_globe() else {
            return;
        };
        globe.camera.target_lon_deg = 180.0;
        globe.camera.target_lat_deg = 0.0;
        globe.set_image_overlays(vec![image_overlay(
            "pacific",
            (170.0, -5.0, -170.0, 5.0),
            solid(32, 32, BLUE),
        )]);
        for lon in [172.0, 180.0, -175.0] {
            assert_close(at(&mut globe, lon, 0.0), BLUE, 2);
        }
        assert_close(at(&mut globe, 168.5, 0.0), GREEN, 2);
        assert_close(at(&mut globe, -168.5, 0.0), GREEN, 2);
    }

    #[test]
    fn image_overlay_outside_the_view_is_harmless() {
        let Some(mut globe) = overlay_globe() else {
            return;
        };
        let (_, base) = frame(&mut globe);
        globe.set_image_overlays(vec![
            image_overlay("far", (170.0, -30.0, 175.0, -25.0), solid(8, 8, BLUE)),
            image_overlay("nowhere", (0.0, 0.0, 0.0, 0.0), solid(8, 8, BLUE)),
            image_overlay("empty", (0.0, 0.0, 1.0, 1.0), RgbaImage::new(0, 0)),
        ]);
        assert_eq!(
            globe.renderer.image_overlay_count(),
            1,
            "unusable ones are skipped"
        );
        let (_, after) = frame(&mut globe);
        assert_eq!(base, after);
    }

    #[test]
    fn unchanged_images_are_not_uploaded_again() {
        let Some(mut globe) = overlay_globe() else {
            return;
        };
        let overlay = near_target(solid(16, 16, BLUE));
        globe.set_image_overlays(vec![overlay.clone()]);
        assert_eq!(globe.renderer.image_upload_count(), 1);

        globe.set_image_overlays(vec![ImageOverlay {
            opacity: 0.5,
            rotation_deg: 20.0,
            west: 8.0,
            ..overlay.clone()
        }]);
        assert_eq!(
            globe.renderer.image_upload_count(),
            1,
            "bounds, rotation and opacity changes reuse the texture"
        );

        globe.set_image_overlays(vec![ImageOverlay {
            image: Arc::new(solid(16, 16, RED)),
            ..overlay
        }]);
        assert_eq!(
            globe.renderer.image_upload_count(),
            2,
            "a new picture uploads"
        );
    }

    #[test]
    fn tile_layers_are_matched_by_source_when_the_list_changes() {
        let Some(mut globe) = overlay_globe() else {
            return;
        };
        globe.set_tile_overlays(vec![layer("a", 1.0), layer("b", 1.0)]);
        ingest_layer(&mut globe, "a", RED);
        ingest_layer(&mut globe, "b", BLUE);

        globe.set_tile_overlays(vec![layer("b", 0.4), layer("a", 0.7)]);
        assert_eq!(globe.renderer.tile_overlay_ids(), ["b", "a"]);
        assert!(globe.renderer.has_overlay_texture("a", TileId::root()));
        assert!(globe.renderer.has_overlay_texture("b", TileId::root()));

        globe.set_tile_overlays(vec![layer("b", 0.4)]);
        assert_eq!(globe.renderer.tile_overlay_ids(), ["b"]);
        assert!(!globe.renderer.has_overlay_texture("a", TileId::root()));
        assert!(globe.renderer.has_overlay_texture("b", TileId::root()));

        let mut edited = layer("b", 0.4);
        edited.source.url = "http://127.0.0.1:1/other/{z}/{x}/{y}.png".to_string();
        globe.set_tile_overlays(vec![edited]);
        assert!(
            !globe.renderer.has_overlay_texture("b", TileId::root()),
            "a different source under the same id must not show the old imagery"
        );

        globe.set_tile_overlays(vec![layer("c", 1.0), layer("c", 0.5)]);
        assert_eq!(
            globe.renderer.tile_overlay_ids(),
            ["c"],
            "repeated ids collapse"
        );
    }

    #[test]
    fn credits_list_the_basemap_and_the_visible_layers_once() {
        let Some(mut globe) = overlay_globe() else {
            return;
        };
        let basemap = globe.provider.credit().to_string();
        assert_eq!(globe.credits(), std::slice::from_ref(&basemap));
        globe.set_tile_overlays(vec![
            layer("a", 1.0),
            layer("hidden", 0.0),
            layer("a", 1.0),
            layer("b", 0.3),
        ]);
        assert_eq!(
            globe.credits(),
            [
                basemap,
                "credit of a".to_string(),
                "credit of b".to_string()
            ]
        );
    }

    #[test]
    fn scene_fills_stay_above_every_overlay() {
        let Some(mut globe) = overlay_globe() else {
            return;
        };
        let center = terra_core::GeoPoint::new(TARGET.0, TARGET.1).expect("point");
        globe.set_scene(
            1,
            terra_globe::Scene {
                entries: vec![terra_globe::SceneEntry {
                    id: "disc".into(),
                    geometry: terra_core::PlacemarkGeometry::circle(center, 60_000.0)
                        .expect("circle"),
                    style: red_fill_style(),
                    visible: true,
                    selected: false,
                }],
                ..Default::default()
            },
        );
        globe.set_tile_overlays(vec![layer("blue", 1.0)]);
        ingest_layer(&mut globe, "blue", BLUE);
        globe.set_image_overlays(vec![near_target(solid(16, 16, [0, 255, 0, 255]))]);

        let pixel = centre(&mut globe);
        assert!(
            pixel[0] > 180 && pixel[1] < 120 && pixel[2] < 120,
            "the fill must cover both overlays, got {pixel:?}"
        );
        let beside = at(&mut globe, 10.0, 35.6);
        assert_close(beside, [0, 255, 0, 255], 3);
    }

    #[test]
    fn overlays_work_in_the_flat_views() {
        let Some(mut globe) = overlay_globe() else {
            return;
        };
        globe.flat.fly_to(
            &terra_core::GeoPoint::new(TARGET.0, TARGET.1).expect("point"),
            2_000_000.0,
        );
        globe.set_mode(SceneMode::Scene2D);
        globe.set_tile_overlays(vec![layer("red", 0.5)]);
        ingest_layer(&mut globe, "red", RED);
        assert_close(centre(&mut globe), mix(RED, GREEN, 0.5), 3);

        globe.set_image_overlays(vec![near_target(solid(16, 16, BLUE))]);
        assert_close(centre(&mut globe), BLUE, 2);
        assert_close(at(&mut globe, 13.0, 35.0), mix(RED, GREEN, 0.5), 3);

        globe.set_scene(
            1,
            terra_globe::Scene {
                entries: vec![terra_globe::SceneEntry {
                    id: "disc".into(),
                    geometry: terra_core::PlacemarkGeometry::circle(
                        terra_core::GeoPoint::new(TARGET.0, TARGET.1).expect("point"),
                        60_000.0,
                    )
                    .expect("circle"),
                    style: red_fill_style(),
                    visible: true,
                    selected: false,
                }],
                ..Default::default()
            },
        );
        let pixel = centre(&mut globe);
        assert!(
            pixel[0] > 180 && pixel[1] < 120 && pixel[2] < 120,
            "{pixel:?}"
        );

        globe.set_mode(SceneMode::Columbus);
        globe.render(0.0);
        globe.set_mode(SceneMode::Scene3D);
        assert_close(centre(&mut globe), RED, 255);
    }

    #[test]
    fn image_overlay_across_the_antimeridian_shows_on_both_flat_edges() {
        let Some(mut globe) = overlay_globe() else {
            return;
        };
        globe.set_mode(SceneMode::Scene2D);
        globe.set_image_overlays(vec![image_overlay(
            "pacific",
            (170.0, -5.0, -170.0, 5.0),
            solid(32, 32, BLUE),
        )]);
        for lon in [175.0, -175.0] {
            globe.flat.fly_to(
                &terra_core::GeoPoint::new(lon, 0.0).expect("point"),
                1_000_000.0,
            );
            assert_close(centre(&mut globe), BLUE, 2);
        }
    }
}
