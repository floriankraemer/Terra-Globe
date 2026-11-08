//! Terrain rendering, picking and camera behaviour, headless and offline.
//!
//! A synthetic 4 km mountain is injected as elevation tiles, so nothing here
//! touches the network. Imagery and the elevation source both point at an
//! address that never answers.

use image::RgbaImage;
use terra_core::{GeoPoint, PlacemarkGeometry};
use terra_globe::geo::{Geodetic, TileId};
use terra_globe::imagery::{FetchedTile, TILE_TEXTURE_SIZE, TileProvider};
use terra_globe::{Globe, Scene, SceneEntry, TerrainTile};
use terra_providers::TileSource;

const SUMMIT: (f64, f64) = (10.0, 35.0);
const SUMMIT_HEIGHT: f64 = 4_000.0;

fn headless_device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .ok()?;
    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).ok()
}

fn checkerboard(seed: u32) -> RgbaImage {
    let mut image = RgbaImage::new(TILE_TEXTURE_SIZE, TILE_TEXTURE_SIZE);
    for (x, y, pixel) in image.enumerate_pixels_mut() {
        let checker = ((x / 16) + (y / 16) + seed).is_multiple_of(2);
        *pixel = if checker {
            image::Rgba([220, 210, 190, 255])
        } else {
            image::Rgba([40, 90, 140, 255])
        };
    }
    image
}

fn offline(mut source: TileSource) -> TileSource {
    source.url = "http://127.0.0.1:1/{z}/{x}/{y}.png".to_string();
    source
}

/// A Gaussian mountain, 4 km high and about 6 km wide, on flat sea level.
fn mountain(lon: f64, lat: f64) -> f64 {
    let east = (lon - SUMMIT.0) * lat.to_radians().cos() * 111_320.0;
    let north = (lat - SUMMIT.1) * 110_574.0;
    SUMMIT_HEIGHT * (-(east * east + north * north) / (6_000.0 * 6_000.0)).exp()
}

/// The 3x3 level-9 elevation tiles around the summit.
fn inject_mountain(globe: &mut Globe) {
    let centre = tile_containing(9, SUMMIT.0, SUMMIT.1);
    for dy in -1i64..=1 {
        for dx in -1i64..=1 {
            let id = TileId::new(
                9,
                (i64::from(centre.x) + dx) as u32,
                (i64::from(centre.y) + dy) as u32,
            )
            .expect("neighbour tile");
            globe.inject_terrain_tile(id, TerrainTile::synthetic(id, mountain));
        }
    }
}

fn tile_containing(z: u8, lon: f64, lat: f64) -> TileId {
    let (u, v) = terra_globe::geo::lon_lat_to_mercator_uv(lon, lat);
    let count = 2f64.powi(i32::from(z));
    TileId::new(z, (u * count) as u32, (v * count) as u32).expect("tile")
}

/// Looks north at the mountain from 19 km away and a little above the plain.
fn looking_at_the_mountain(globe: &mut Globe) {
    globe.camera.target_lon_deg = SUMMIT.0;
    globe.camera.target_lat_deg = 34.88;
    globe.camera.distance = 6_000.0;
    globe.camera.heading = 0.0;
    globe.camera.pitch = 12f64.to_radians();
}

fn globe(terrain: bool) -> Option<Globe> {
    let (device, queue) = headless_device()?;
    let mut globe = Globe::new(
        device,
        queue,
        TileProvider::new(offline(TileSource::open_street_map())),
        (800, 500),
    );
    let mut tiles = vec![TileId::root()];
    tiles.extend(TileId::root().children().into_iter().flatten());
    globe.renderer.ingest(
        tiles
            .into_iter()
            .map(|id| FetchedTile {
                id,
                result: Ok(checkerboard(u32::from(id.z) + id.x + id.y)),
            })
            .collect(),
    );
    globe.set_terrain_source(offline(TileSource::terrain_terrarium()));
    if terrain {
        inject_mountain(&mut globe);
        globe.set_terrain_enabled(true);
    }
    looking_at_the_mountain(&mut globe);
    globe.settle_camera();
    Some(globe)
}

fn pixels(globe: &mut Globe) -> (usize, usize, Vec<u8>) {
    globe.render(0.0);
    let (width, height, pixels) = globe.renderer.read_pixels().expect("readback");
    (width as usize, height as usize, pixels)
}

fn is_background(pixel: &[u8]) -> bool {
    pixel[0] < 8 && pixel[1] < 8 && pixel[2] < 14
}

fn is_red(pixel: &[u8]) -> bool {
    pixel[0] > 170 && pixel[1] < 90 && pixel[2] < 90
}

/// First row from the top, in the middle columns, that is not sky.
fn skyline(width: usize, height: usize, data: &[u8]) -> usize {
    (0..height)
        .find(|&row| {
            (width / 2 - 20..width / 2 + 20)
                .any(|column| !is_background(&data[(row * width + column) * 4..][..4]))
        })
        .unwrap_or(height)
}

fn red_line_through_the_summit() -> Scene {
    let line = PlacemarkGeometry::line_string(
        vec![
            GeoPoint::new(SUMMIT.0, 34.90).expect("point"),
            GeoPoint::new(SUMMIT.0, 35.10).expect("point"),
        ],
        None,
        None,
    )
    .expect("line");
    Scene {
        entries: vec![SceneEntry {
            id: "trail".into(),
            geometry: line,
            style: terra_core::Style {
                outline_enabled: true,
                outline_color: "#ff1100".into(),
                outline_width: 5.0,
                filled: false,
                ..terra_core::Style::default()
            },
            visible: true,
            selected: false,
        }],
        ..Default::default()
    }
}

fn differing(first: &[u8], second: &[u8], tolerance: i16) -> usize {
    first
        .chunks_exact(4)
        .zip(second.chunks_exact(4))
        .filter(|(a, b)| {
            a.iter()
                .zip(b.iter())
                .any(|(x, y)| (i16::from(*x) - i16::from(*y)).abs() > tolerance)
        })
        .count()
}

#[test]
fn a_mountain_changes_the_silhouette() {
    let (Some(mut flat), Some(mut hilly)) = (globe(false), globe(true)) else {
        eprintln!("no GPU adapter available; skipping terrain silhouette test");
        return;
    };
    let (width, height, flat_pixels) = pixels(&mut flat);
    let (_, _, hilly_pixels) = pixels(&mut hilly);
    let flat_skyline = skyline(width, height, &flat_pixels);
    let hilly_skyline = skyline(width, height, &hilly_pixels);
    assert!(
        hilly_skyline + 40 < flat_skyline,
        "the mountain should rise {} rows above the flat horizon, got {hilly_skyline} vs {flat_skyline}",
        40
    );
}

#[test]
fn a_clamped_line_over_the_mountain_is_visible_not_buried() {
    let Some(mut globe) = globe(true) else {
        return;
    };
    globe.set_scene(1, red_line_through_the_summit());
    let (width, _, data) = pixels(&mut globe);
    let red = data.chunks_exact(4).filter(|p| is_red(p)).count();
    assert!(red > 150, "the trail is mostly hidden: {red} red pixels");

    // Right where the trail crosses the summit slope, on the mountain itself.
    let ground = globe
        .ground_height(SUMMIT.0, 34.97)
        .expect("terrain loaded");
    assert!(ground > 1_000.0, "34.97 is on the south slope: {ground}");
    let screen = globe
        .project(Geodetic::from_degrees(SUMMIT.0, 34.97, ground))
        .expect("on screen");
    let window = (-4i32..=4).flat_map(|dy| (-4i32..=4).map(move |dx| (dx, dy)));
    let hits = window
        .filter(|(dx, dy)| {
            let x = (screen.x as i32 + dx) as usize;
            let y = (screen.y as i32 + dy) as usize;
            is_red(&data[(y * width + x) * 4..][..4])
        })
        .count();
    assert!(hits > 5, "no trail pixels on the slope at {screen:?}");
}

#[test]
fn turning_terrain_off_restores_the_flat_surface_and_drops_placemarks_back() {
    let (Some(mut reference), Some(mut toggled)) = (globe(false), globe(true)) else {
        return;
    };
    reference.set_scene(1, red_line_through_the_summit());
    toggled.set_scene(1, red_line_through_the_summit());
    let (_, _, flat) = pixels(&mut reference);
    let (_, _, hilly) = pixels(&mut toggled);
    assert!(differing(&flat, &hilly, 2) > 500, "terrain must be visible");

    toggled.set_terrain_enabled(false);
    assert_eq!(toggled.camera.target_height, 0.0);
    assert_eq!(toggled.ground_height(SUMMIT.0, SUMMIT.1), None);
    let (_, _, restored) = pixels(&mut toggled);
    assert_eq!(
        differing(&flat, &restored, 0),
        0,
        "switching terrain off must give back exactly the flat frame"
    );

    toggled.set_terrain_enabled(true);
    let (_, _, again) = pixels(&mut toggled);
    assert!(differing(&flat, &again, 2) > 500, "and back on again");
}

#[test]
fn failed_elevation_fetches_leave_the_ellipsoid_intact_without_holes() {
    let (Some(mut flat), Some(mut offline_terrain)) = (globe(false), globe(false)) else {
        return;
    };
    offline_terrain.set_terrain_enabled(true);
    let (width, height, reference) = pixels(&mut flat);
    let mut data = Vec::new();
    for _ in 0..5 {
        data = pixels(&mut offline_terrain).2;
        std::thread::sleep(std::time::Duration::from_millis(30));
    }
    assert!(offline_terrain.renderer.stats().terrain_tiles == 0);
    assert!(offline_terrain.ground_height(SUMMIT.0, SUMMIT.1).is_none());
    // Everything below the horizon is globe, never the clear colour.
    let first_ground_row = skyline(width, height, &reference);
    for row in first_ground_row + 4..height {
        for column in 0..width {
            assert!(
                !is_background(&data[(row * width + column) * 4..][..4]),
                "hole at ({column}, {row})"
            );
        }
    }
    let changed = differing(&reference, &data, 40);
    assert!(
        changed * 200 < width * height,
        "{changed} pixels differ from the plain ellipsoid"
    );
}

#[test]
fn picking_lands_on_the_mountain_not_on_the_ellipsoid_behind_it() {
    let (Some(mut hilly), Some(mut flat)) = (globe(true), globe(false)) else {
        return;
    };
    hilly.render(0.0);
    flat.render(0.0);
    let summit = hilly
        .project(Geodetic::from_degrees(SUMMIT.0, SUMMIT.1, SUMMIT_HEIGHT))
        .expect("summit on screen");
    let pixel = (summit.x, summit.y + 6.0);

    let on_terrain = hilly.pick(pixel).expect("hits the mountain");
    assert!(
        on_terrain.height_m > 3_000.0,
        "picked {} m, the summit region is 4000 m",
        on_terrain.height_m
    );
    // With terrain off that pixel looks over the mountain into the sky.
    assert!(
        flat.pick(pixel).is_none(),
        "the ellipsoid pick must miss: the summit is above the flat horizon"
    );
}

#[test]
fn markers_are_selected_where_the_terrain_draws_them() {
    let Some(mut globe) = globe(true) else {
        return;
    };
    let marker = PlacemarkGeometry::point(GeoPoint::new(SUMMIT.0, SUMMIT.1).expect("point"))
        .expect("marker");
    globe.set_scene(
        1,
        Scene {
            entries: vec![SceneEntry {
                id: "summit".into(),
                geometry: marker,
                style: terra_core::Style::default(),
                visible: true,
                selected: false,
            }],
            ..Default::default()
        },
    );
    globe.render(0.0);
    let ground = globe.ground_height(SUMMIT.0, SUMMIT.1).expect("loaded");
    assert!(ground > 3_500.0, "{ground}");
    let drawn = globe
        .project(Geodetic::from_degrees(SUMMIT.0, SUMMIT.1, ground))
        .expect("on screen");
    let hit = globe.pick_placemark((drawn.x, drawn.y));
    assert_eq!(hit.map(|hit| hit.id), Some("summit".to_string()));
    let at_sea_level = globe
        .project(Geodetic::from_degrees(SUMMIT.0, SUMMIT.1, 0.0))
        .expect("on screen");
    assert!(
        globe
            .pick_placemark((at_sea_level.x, at_sea_level.y))
            .is_none(),
        "the marker is not at sea level any more"
    );
}

#[test]
fn the_eye_never_goes_below_the_ground() {
    let Some(mut globe) = globe(true) else {
        return;
    };
    // The target is north of the summit and the eye ends up over it, but low.
    globe.camera.target_lat_deg = 35.1;
    globe.camera.distance = 9_000.0;
    globe.camera.pitch = 10f64.to_radians();
    globe.render(0.0);
    let eye = terra_globe::ecef_to_geodetic(globe.eye_ecef());
    let ground = globe
        .ground_height(eye.lon_deg(), eye.lat_deg())
        .expect("ground under the eye");
    assert!(
        eye.height >= ground + terra_globe::camera::EYE_CLEARANCE_M - 0.01,
        "eye at {} m over ground at {ground} m",
        eye.height
    );

    // Without terrain the same pose is allowed to sit at its natural height.
    let Some(mut plain) = globe_without_terrain_at_the_same_pose() else {
        return;
    };
    plain.render(0.0);
    assert!(plain.camera.distance < 9_000.0 * 1.0001);
}

fn globe_without_terrain_at_the_same_pose() -> Option<Globe> {
    let mut plain = globe(false)?;
    plain.camera.target_lat_deg = 35.1;
    plain.camera.distance = 9_000.0;
    plain.camera.pitch = 10f64.to_radians();
    Some(plain)
}

#[test]
fn zooming_towards_the_summit_closes_in_on_the_summit() {
    let Some(mut globe) = globe(true) else {
        return;
    };
    globe.render(0.0);
    let summit = Geodetic::from_degrees(SUMMIT.0, SUMMIT.1, SUMMIT_HEIGHT);
    let range = |globe: &Globe| (terra_globe::geodetic_to_ecef(summit) - globe.eye_ecef()).length();
    let start = range(&globe);
    for _ in 0..5 {
        let screen = globe.project(summit).expect("summit stays on screen");
        globe.zoom_at((screen.x, screen.y + 4.0), 0.6);
        globe.render(0.0);
    }
    assert!(
        range(&globe) < start * 0.3,
        "the eye is {} m from the summit, started {start} m",
        range(&globe)
    );
    let ground = globe
        .ground_height(globe.camera.target_lon_deg, globe.camera.target_lat_deg)
        .expect("loaded");
    assert!(
        (globe.camera.target_height - ground).abs() < 3.0,
        "the pivot {} sits on the ground {ground}",
        globe.camera.target_height
    );
}

#[test]
fn flat_scene_modes_ignore_terrain() {
    let Some(mut globe) = globe(true) else {
        return;
    };
    assert!(globe.terrain_active());
    assert!(
        globe
            .credits()
            .iter()
            .any(|credit| credit.contains("Mapzen Terrain Tiles"))
    );
    globe.set_mode(terra_globe::SceneMode::Scene2D);
    assert!(!globe.terrain_active());
    assert_eq!(globe.ground_height(SUMMIT.0, SUMMIT.1), None);
    assert!(
        !globe
            .credits()
            .iter()
            .any(|credit| credit.contains("Mapzen Terrain Tiles"))
    );
    globe.render(0.0);
    assert_eq!(globe.renderer.stats().terrain_requests, 0);
    globe.set_mode(terra_globe::SceneMode::Scene3D);
    assert!(globe.ground_height(SUMMIT.0, SUMMIT.1).is_some());
}

#[test]
fn terrain_is_off_for_a_new_globe_and_only_asks_for_tiles_once_on() {
    let Some(mut globe) = globe(false) else {
        return;
    };
    assert!(!globe.terrain_enabled());
    globe.render(0.0);
    assert_eq!(globe.renderer.stats().terrain_requests, 0);
    assert!(
        !globe
            .credits()
            .iter()
            .any(|credit| credit.contains("Mapzen Terrain Tiles"))
    );
    globe.set_terrain_enabled(true);
    globe.render(0.0);
    assert!(
        globe.renderer.stats().terrain_requests > 0,
        "elevation tiles are requested for the visible imagery"
    );
}

#[test]
fn drawn_tiles_are_stitched_to_their_coarser_neighbours() {
    let Some(mut globe) = globe(true) else {
        return;
    };
    // Frames until the rebuild budget has caught up with the selection.
    for _ in 0..6 {
        globe.render(0.0);
    }
    let selection: Vec<TileId> = globe
        .renderer
        .last_selection()
        .iter()
        .map(|tile| tile.id)
        .collect();
    let drawn: std::collections::HashSet<TileId> = selection.iter().copied().collect();
    let mut stitched = 0;
    for id in &selection {
        let expected = terra_globe::coarser_neighbours(*id, &|tile| drawn.contains(&tile));
        assert_eq!(
            globe.renderer.mesh_stitching(*id),
            Some(expected),
            "tile {id} is not stitched to what is drawn next to it"
        );
        stitched += expected.iter().filter(|levels| **levels > 0).count();
    }
    assert!(
        stitched > 0,
        "a view across many levels of detail must have stitched edges"
    );
}

#[test]
fn flat_meshes_are_never_stitched() {
    let Some(mut globe) = globe(false) else {
        return;
    };
    for _ in 0..3 {
        globe.render(0.0);
    }
    for tile in globe.renderer.last_selection() {
        assert_eq!(globe.renderer.mesh_stitching(tile.id), Some([0; 4]));
    }
}
