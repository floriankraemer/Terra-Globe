//! Renders the globe from a local tile cache, for eyeballing real imagery.
//!
//! Tiles are served from `$TERRA_TILE_CACHE/<z>/<x>/<y>.png` by a loopback HTTP
//! server, so the normal fetch path (and its LOD/fallback behaviour) is used.
//! Missing tiles are listed at the end so they can be fetched separately.

use std::io::Read;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use terra_globe::imagery::TileProvider;
use terra_globe::{Camera, Globe};

fn main() {
    let cache: PathBuf = std::env::var("TERRA_TILE_CACHE")
        .unwrap_or_else(|_| "/tmp/terra-tiles".to_string())
        .into();
    let out = std::env::var("TERRA_TILE_OUT").unwrap_or_else(|_| "/tmp/terra-render.png".into());
    let width: u32 = std::env::var("TERRA_VIEW_W")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1280);
    let height: u32 = std::env::var("TERRA_VIEW_H")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(800);
    let lon: f64 = std::env::var("TERRA_CAM_LON")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(-10.0);
    let lat: f64 = std::env::var("TERRA_CAM_LAT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(35.0);
    let distance: f64 = std::env::var("TERRA_CAM_DISTANCE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(8_000_000.0);
    let pitch: f64 = std::env::var("TERRA_CAM_PITCH")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(45.0);
    let heading: f64 = std::env::var("TERRA_CAM_HEADING")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.0);

    let missing = Arc::new(Mutex::new(Vec::new()));
    let requests = Arc::new(Mutex::new(0usize));

    let server = tiny_http::Server::http("127.0.0.1:0").expect("bind");
    let port = server.server_addr().to_ip().expect("tcp").port();
    {
        let cache = cache.clone();
        let missing = missing.clone();
        let requests = requests.clone();
        std::thread::spawn(move || {
            for request in server.incoming_requests() {
                *requests.lock().expect("requests") += 1;
                let path = request.url().trim_start_matches('/').to_string();
                let file = cache.join(&path);
                let mut bytes = Vec::new();
                let served = std::fs::File::open(&file)
                    .and_then(|mut file| file.read_to_end(&mut bytes))
                    .is_ok();
                if !served {
                    missing.lock().expect("missing").push(path.clone());
                    let _ = request.respond(tiny_http::Response::empty(404));
                    continue;
                }
                let response = tiny_http::Response::from_data(bytes).with_header(
                    "Content-Type: image/png"
                        .parse::<tiny_http::Header>()
                        .expect("header"),
                );
                let _ = request.respond(response);
            }
        });
    }

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .expect("no GPU adapter");
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
            .expect("no device");

    let mut source = terra_providers::TileSource::open_street_map();
    source.url = format!("http://127.0.0.1:{port}/{{z}}/{{x}}/{{y}}.png");
    let mut globe = Globe::new(device, queue, TileProvider::new(source), (width, height));
    let mut camera = Camera::default();
    camera.target_lon_deg = lon;
    camera.target_lat_deg = lat;
    camera.distance = distance;
    camera.pitch = pitch.to_radians();
    camera.heading = heading.to_radians();
    globe.camera = camera;

    let zoom_to: Option<f64> = std::env::var("TERRA_ZOOM_TO")
        .ok()
        .and_then(|v| v.parse().ok());

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    let mut idle_frames = 0;
    let mut frames = 0;
    let mut zoomed = zoom_to.is_none();
    while std::time::Instant::now() < deadline {
        let stats = globe.render(frames as f64 * 0.016);
        frames += 1;
        if stats.requests_in_flight == 0 && stats.requests_queued == 0 {
            idle_frames += 1;
            if idle_frames == 20 {
                println!(
                    "settled after {frames} frames: deepest level {}, {} textures",
                    stats.deepest_level, stats.textures_ready
                );
            }
            if idle_frames > 20 {
                if zoomed {
                    break;
                }
                globe.camera.distance = zoom_to.expect("zoom distance");
                zoomed = true;
                idle_frames = 0;
            }
        } else {
            idle_frames = 0;
        }
        std::thread::sleep(std::time::Duration::from_millis(16));
    }

    let stats = globe.renderer.stats();
    let levels = globe.renderer.last_selection();
    let mut per_level: std::collections::BTreeMap<u8, usize> = std::collections::BTreeMap::new();
    for tile in levels {
        *per_level.entry(tile.id.z).or_default() += 1;
    }
    println!(
        "frames {frames}, {} requests, {} tiles {:?}, {} textures",
        requests.lock().expect("requests"),
        levels.len(),
        per_level,
        stats.textures_ready
    );

    let missing = missing.lock().expect("missing");
    let mut missing_sorted: Vec<&String> = missing.iter().collect();
    missing_sorted.sort();
    missing_sorted.dedup();
    for path in &missing_sorted {
        println!("MISSING {path}");
    }

    let (w, h, pixels) = globe.renderer.read_pixels().expect("readback");
    std::fs::write(&out, terra_testkit::rgba_to_png(&pixels, w, h)).expect("write png");
    println!("wrote {out}");
}
