//! Prints what the tile selector picks for a few camera poses.
//!
//! `cargo run -p terra-globe --example lod_probe` is a diagnostic helper for
//! looking at level distribution and coverage without a GPU or a window.

use std::collections::BTreeMap;

use terra_globe::geo::{Geodetic, TileId, geodetic_to_ecef, is_point_visible};
use terra_globe::{Camera, SelectionOptions, select_tiles};

fn summary(camera: &Camera, viewport: (u32, u32), options: SelectionOptions) {
    let eye = camera.eye_ecef();
    let tiles = select_tiles(camera, viewport, 19, options, &|_| false);
    let mut per_level: BTreeMap<u8, usize> = BTreeMap::new();
    for tile in &tiles {
        *per_level.entry(tile.id.z).or_default() += 1;
    }

    let mut uncovered = 0;
    let mut covered = 0;
    let mut lon = -178.0;
    while lon <= 180.0 {
        let mut lat = -84.0;
        while lat <= 84.0 {
            let point = geodetic_to_ecef(Geodetic::from_degrees(lon, lat, 0.0));
            let on_screen = camera.project(point, viewport).is_some_and(|p| {
                (0.02..0.98).contains(&(p.x / f64::from(viewport.0)))
                    && (0.02..0.98).contains(&(p.y / f64::from(viewport.1)))
            });
            if on_screen && is_point_visible(eye, point) {
                let hits = tiles
                    .iter()
                    .filter(|tile| contains(tile.id, lon, lat))
                    .count();
                if hits == 1 {
                    covered += 1;
                } else {
                    uncovered += 1;
                    if std::env::var_os("TERRA_PROBE_VERBOSE").is_some() {
                        let p = camera.project(point, viewport).expect("on screen");
                        println!(
                            "  MISS lon {lon:>7.1} lat {lat:>6.1} screen ({:>6.1},{:>6.1}) hits {hits}",
                            p.x, p.y
                        );
                    }
                }
            }
            lat += 4.0;
        }
        lon += 4.0;
    }

    println!(
        "distance {:>10.0} m  viewport {:?}  budget {}  -> {} tiles {:?}  covered {} / {} on-screen samples",
        camera.distance,
        viewport,
        options.max_tiles,
        tiles.len(),
        per_level,
        covered,
        covered + uncovered
    );
}

fn contains(id: TileId, lon: f64, lat: f64) -> bool {
    let bounds = id.bounds();
    lon >= bounds.west && lon < bounds.east && lat >= bounds.south && lat < bounds.north
}

fn main() {
    let mut camera = Camera::default();
    camera.target_lon_deg = 10.0;
    camera.target_lat_deg = 50.0;
    camera.distance = 400_000.0;
    camera.pitch = 55f64.to_radians();
    let runs = 200;
    let start = std::time::Instant::now();
    let mut last = 0;
    for _ in 0..runs {
        last = select_tiles(
            &camera,
            (1280, 800),
            19,
            SelectionOptions::default(),
            &|_| false,
        )
        .len();
    }
    println!(
        "selection: {last} tiles, {:.2} ms per call",
        start.elapsed().as_secs_f64() * 1000.0 / f64::from(runs)
    );

    for distance in [
        40_000_000.0,
        20_000_000.0,
        8_000_000.0,
        2_000_000.0,
        200_000.0,
    ] {
        let mut camera = Camera::default();
        camera.target_lon_deg = -10.0;
        camera.target_lat_deg = 35.0;
        camera.distance = distance;
        camera.pitch = 45f64.to_radians();
        summary(&camera, (1280, 800), SelectionOptions::default());
    }
}
