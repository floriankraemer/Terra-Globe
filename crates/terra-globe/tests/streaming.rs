//! End-to-end tile streaming: a local HTTP server serves generated tiles, and
//! the globe is zoomed the way a user would. The test asserts that deeper zoom
//! levels actually arrive, which is what "streaming" means.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use terra_globe::imagery::TileProvider;
use terra_globe::{Camera, Globe};
use terra_providers::TileSource;

struct TileServer {
    url: String,
    requests: Arc<AtomicUsize>,
}

fn spawn_tile_server() -> TileServer {
    let server = tiny_http::Server::http("127.0.0.1:0").expect("bind tile server");
    let port = server.server_addr().to_ip().expect("tcp addr").port();
    let requests = Arc::new(AtomicUsize::new(0));
    let counter = requests.clone();

    std::thread::spawn(move || {
        let png = {
            let mut image = image::RgbaImage::new(256, 256);
            for (x, y, pixel) in image.enumerate_pixels_mut() {
                let checker = ((x / 32) + (y / 32)).is_multiple_of(2);
                *pixel = if checker {
                    image::Rgba([200, 180, 150, 255])
                } else {
                    image::Rgba([40, 90, 140, 255])
                };
            }
            let mut bytes = std::io::Cursor::new(Vec::new());
            image::DynamicImage::ImageRgba8(image)
                .write_to(&mut bytes, image::ImageFormat::Png)
                .expect("encode png");
            bytes.into_inner()
        };

        for request in server.incoming_requests() {
            counter.fetch_add(1, Ordering::Relaxed);
            let response = tiny_http::Response::from_data(png.clone()).with_header(
                "Content-Type: image/png"
                    .parse::<tiny_http::Header>()
                    .expect("header"),
            );
            let _ = request.respond(response);
        }
    });

    TileServer {
        url: format!("http://127.0.0.1:{port}/{{z}}/{{x}}/{{y}}.png"),
        requests,
    }
}

fn headless_globe(server: &TileServer) -> Option<Globe> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .ok()?;
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).ok()?;

    let mut source = TileSource::open_street_map();
    source.url = server.url.clone();
    source.maximum_level = 19;
    let mut globe = Globe::new(device, queue, TileProvider::new(source), (600, 400));
    let mut camera = Camera::default();
    camera.target_lon_deg = 10.0;
    camera.target_lat_deg = 35.0;
    camera.distance = 25_000_000.0;
    camera.pitch = 50f64.to_radians();
    globe.camera = camera;
    Some(globe)
}

/// Renders until `condition` holds or the deadline passes.
fn pump(globe: &mut Globe, deadline: Duration, mut condition: impl FnMut(&Globe) -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < deadline {
        globe.render(start.elapsed().as_secs_f64());
        if condition(globe) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

#[test]
fn zooming_in_streams_deeper_tiles() {
    let server = spawn_tile_server();
    let Some(mut globe) = headless_globe(&server) else {
        eprintln!("no GPU adapter; skipping streaming test");
        return;
    };

    // First paint: at least one texture arrives.
    assert!(
        pump(&mut globe, Duration::from_secs(10), |globe| {
            !globe.renderer.ready_texture_levels().is_empty()
        }),
        "no tile texture arrived at all (server saw {} requests)",
        server.requests.load(Ordering::Relaxed)
    );
    let shallow = globe.renderer.ready_texture_levels();
    assert!(
        shallow.last().copied().unwrap_or(0) <= 3,
        "the zoomed-out view should start with coarse tiles, got {shallow:?}"
    );

    // Zoom in hard, as a user would: the renderer must request and upload
    // deeper levels instead of stretching the coarse texture forever.
    globe.camera.distance = 40_000.0;
    globe.camera.pitch = 60f64.to_radians();

    let deep = pump(&mut globe, Duration::from_secs(25), |globe| {
        globe
            .renderer
            .ready_texture_levels()
            .last()
            .copied()
            .unwrap_or(0)
            >= 11
    });
    let levels = globe.renderer.ready_texture_levels();
    assert!(
        deep,
        "deeper tiles never loaded after zooming in. shallow={shallow:?} now={levels:?}, server requests={}",
        server.requests.load(Ordering::Relaxed)
    );

    // Once the fetcher goes idle, every drawn tile must come from its own
    // texture. A leftover ancestor fallback is exactly the "sharp here, blurry
    // there" symptom: the frame is complete but parts of it are still a
    // magnified coarse tile.
    let settled = pump(&mut globe, Duration::from_secs(30), |globe| {
        let stats = globe.renderer.stats();
        stats.requests_in_flight == 0
            && stats.requests_queued == 0
            && !globe.renderer.last_selection().is_empty()
            && globe
                .renderer
                .last_selection()
                .iter()
                .all(|tile| tile.texture == Some((tile.id, [0.0, 0.0, 1.0, 1.0])))
    });
    let fallbacks: Vec<String> = if settled {
        Vec::new()
    } else {
        globe
            .renderer
            .last_selection()
            .iter()
            .filter(|tile| tile.texture != Some((tile.id, [0.0, 0.0, 1.0, 1.0])))
            .map(|tile| {
                format!(
                    "{} (drawn from {:?})",
                    tile.id,
                    tile.texture.map(|(id, _)| id)
                )
            })
            .collect()
    };
    assert!(
        settled,
        "zoomed-in view never stopped using coarse fallbacks for {} of {} tiles: {:?}",
        fallbacks.len(),
        globe.renderer.last_selection().len(),
        &fallbacks[..fallbacks.len().min(6)]
    );

    // Zooming back out must still resolve textures (coarse ones are cached or
    // re-requested), never leaving the globe blank.
    globe.camera.distance = 25_000_000.0;
    assert!(
        pump(&mut globe, Duration::from_secs(10), |globe| {
            !globe.renderer.ready_texture_levels().is_empty()
        }),
        "no textures available after zooming out"
    );
}

#[test]
fn zooming_requests_more_tiles_than_the_first_paint() {
    let server = spawn_tile_server();
    let Some(mut globe) = headless_globe(&server) else {
        return;
    };
    assert!(pump(&mut globe, Duration::from_secs(10), |globe| {
        !globe.renderer.ready_texture_levels().is_empty()
    }));
    let after_first = server.requests.load(Ordering::Relaxed);

    globe.camera.distance = 40_000.0;
    assert!(pump(&mut globe, Duration::from_secs(25), |globe| {
        globe
            .renderer
            .ready_texture_levels()
            .last()
            .copied()
            .unwrap_or(0)
            >= 11
    }));
    let after_zoom = server.requests.load(Ordering::Relaxed);
    assert!(
        after_zoom > after_first + 4,
        "zooming did not request new tiles (before {after_first}, after {after_zoom})"
    );
}

type CachedTile = (String, u8, u32, u32);

#[derive(Default)]
struct MemoryCache {
    tiles: std::sync::Mutex<std::collections::HashMap<CachedTile, Vec<u8>>>,
}

impl terra_globe::imagery::TileCache for MemoryCache {
    fn get(&self, source: &TileSource, id: terra_globe::TileId) -> Option<Vec<u8>> {
        self.tiles
            .lock()
            .ok()?
            .get(&(source.url.clone(), id.z, id.x, id.y))
            .cloned()
    }

    fn put(&self, source: &TileSource, id: terra_globe::TileId, bytes: &[u8]) {
        if let Ok(mut tiles) = self.tiles.lock() {
            tiles.insert((source.url.clone(), id.z, id.x, id.y), bytes.to_vec());
        }
    }
}

#[test]
fn a_second_session_is_served_from_the_tile_cache() {
    let server = spawn_tile_server();
    let cache = Arc::new(MemoryCache::default());

    let Some(mut first) = headless_globe(&server) else {
        eprintln!("no GPU adapter; skipping cache test");
        return;
    };
    first.set_tile_cache(cache.clone());
    assert!(pump(&mut first, Duration::from_secs(10), |globe| {
        !globe.renderer.ready_texture_levels().is_empty()
    }));
    assert!(
        pump(&mut first, Duration::from_secs(20), |globe| {
            let stats = globe.renderer.stats();
            stats.requests_in_flight == 0 && stats.requests_queued == 0
        }),
        "first session never went idle"
    );
    let after_first = server.requests.load(Ordering::Relaxed);

    let Some(mut second) = headless_globe(&server) else {
        return;
    };
    second.set_tile_cache(cache);
    assert!(
        pump(&mut second, Duration::from_secs(10), |globe| {
            !globe.renderer.ready_texture_levels().is_empty()
        }),
        "the cached tiles never reached the second session"
    );
    assert_eq!(
        server.requests.load(Ordering::Relaxed),
        after_first,
        "a fresh session re-downloaded tiles that were already cached"
    );
}

fn layer_source(server: &TileServer, id: &str) -> TileSource {
    let mut source = TileSource::open_street_map();
    source.id = id.to_string();
    source.url = server.url.clone();
    source
}

fn settle(globe: &mut Globe) {
    assert!(
        pump(globe, Duration::from_secs(30), |globe| {
            !globe.is_loading_imagery()
        }),
        "imagery never went idle"
    );
}

#[test]
fn overlay_layers_stream_independently_and_are_not_refetched_when_restyled() {
    let basemap = spawn_tile_server();
    let layer_server = spawn_tile_server();
    let Some(mut globe) = headless_globe(&basemap) else {
        eprintln!("no GPU adapter; skipping overlay streaming test");
        return;
    };

    globe.set_tile_overlays(vec![terra_globe::TileOverlay {
        source: layer_source(&layer_server, "seamarks"),
        opacity: 1.0,
    }]);
    assert!(
        pump(&mut globe, Duration::from_secs(10), |globe| {
            globe
                .renderer
                .has_overlay_texture("seamarks", terra_globe::geo::TileId::root())
        }),
        "overlay tile never arrived (server saw {} requests)",
        layer_server.requests.load(Ordering::Relaxed)
    );
    settle(&mut globe);
    let requested = layer_server.requests.load(Ordering::Relaxed);
    assert!(requested > 0);

    globe.set_tile_overlays(vec![terra_globe::TileOverlay {
        source: layer_source(&layer_server, "seamarks"),
        opacity: 0.3,
    }]);
    pump(&mut globe, Duration::from_secs(1), |_| false);
    assert_eq!(
        layer_server.requests.load(Ordering::Relaxed),
        requested,
        "an opacity change must not download anything"
    );

    // Hidden layers keep their imagery but stop asking for more.
    globe.set_tile_overlays(vec![terra_globe::TileOverlay {
        source: layer_source(&layer_server, "seamarks"),
        opacity: 0.0,
    }]);
    globe.camera.distance = 200_000.0;
    pump(&mut globe, Duration::from_secs(1), |_| false);
    assert_eq!(layer_server.requests.load(Ordering::Relaxed), requested);

    globe.set_tile_overlays(Vec::new());
    assert!(
        !globe
            .renderer
            .has_overlay_texture("seamarks", terra_globe::geo::TileId::root()),
        "a removed layer must free its textures"
    );
    let after_removal = layer_server.requests.load(Ordering::Relaxed);
    pump(&mut globe, Duration::from_secs(1), |_| false);
    assert_eq!(layer_server.requests.load(Ordering::Relaxed), after_removal);
}

#[test]
fn overlay_layers_follow_the_camera_to_deeper_levels() {
    let basemap = spawn_tile_server();
    let layer_server = spawn_tile_server();
    let Some(mut globe) = headless_globe(&basemap) else {
        return;
    };
    globe.set_tile_overlays(vec![terra_globe::TileOverlay {
        source: layer_source(&layer_server, "deep"),
        opacity: 1.0,
    }]);
    globe.camera.distance = 40_000.0;
    globe.camera.pitch = 60f64.to_radians();
    settle(&mut globe);
    assert!(
        layer_server.requests.load(Ordering::Relaxed) > 4,
        "a close-up view needs several overlay tiles, got {}",
        layer_server.requests.load(Ordering::Relaxed)
    );
}

#[derive(Default)]
struct RecordingCache(std::sync::Mutex<Vec<String>>);

impl terra_globe::TileCache for RecordingCache {
    fn get(&self, source: &TileSource, _id: terra_globe::geo::TileId) -> Option<Vec<u8>> {
        if let Ok(mut seen) = self.0.lock() {
            seen.push(source.id.clone());
        }
        None
    }

    fn put(&self, _source: &TileSource, _id: terra_globe::geo::TileId, _bytes: &[u8]) {}
}

#[test]
fn overlay_layers_share_the_tile_cache_set_before_or_after_them() {
    let basemap = spawn_tile_server();
    let layer_server = spawn_tile_server();
    let Some(mut globe) = headless_globe(&basemap) else {
        return;
    };
    let cache = Arc::new(RecordingCache::default());
    globe.set_tile_overlays(vec![terra_globe::TileOverlay {
        source: layer_source(&layer_server, "early"),
        opacity: 1.0,
    }]);
    globe.set_tile_cache(cache.clone());
    globe.set_tile_overlays(vec![
        terra_globe::TileOverlay {
            source: layer_source(&layer_server, "early"),
            opacity: 1.0,
        },
        terra_globe::TileOverlay {
            source: layer_source(&layer_server, "late"),
            opacity: 1.0,
        },
    ]);
    pump(&mut globe, Duration::from_secs(3), |_| false);
    let seen = cache.0.lock().expect("lock").clone();
    for id in ["early", "late"] {
        assert!(
            seen.iter().any(|seen| seen == id),
            "{id} skipped the cache: {seen:?}"
        );
    }
}
