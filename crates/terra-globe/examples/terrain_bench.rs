//! Times the CPU side of `Globe::render` over a scripted flight across synthetic terrain.
//!
//! `cargo run --release -p terra-globe --example terrain_bench` is the
//! diagnostic for terrain frame cost: no window, no network. Elevation and
//! imagery tiles "arrive" one frame after the view first asks for them, like
//! streaming does, so the mesh rebuilds that arrivals cause are in the numbers.

use image::RgbaImage;
use terra_globe::imagery::{FetchedTile, TILE_TEXTURE_SIZE, TileProvider};
use terra_globe::terrain::elevation_tile_for;
use terra_globe::{Globe, TerrainTile};
use terra_providers::TileSource;

fn offline(mut source: TileSource) -> TileSource {
    source.url = "http://127.0.0.1:1/{z}/{x}/{y}.png".to_string();
    source
}

/// Rough mountains everywhere: ridges of a few kilometres down to 100 m bumps.
fn relief(lon: f64, lat: f64) -> f64 {
    let (x, y) = (lon.to_radians(), lat.to_radians());
    (1500.0 * ((x * 40.0).sin() * (y * 37.0).cos() + 1.0)
        + 600.0 * (x * 313.0 + y * 271.0).sin()
        + 150.0 * (x * 2111.0).sin() * (y * 1873.0).sin())
    .max(0.0)
}

struct Phase {
    name: &'static str,
    frames: u32,
    step: fn(&mut Globe, f64),
}

fn main() {
    // The no-op backend runs everything up to the driver, so the numbers are
    // this crate's CPU work and not a software rasteriser's. `TERRA_BENCH_GPU`
    // uses the machine's real adapter instead.
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    if std::env::var_os("TERRA_BENCH_GPU").is_none() {
        descriptor.backends = wgpu::Backends::NOOP;
        descriptor.backend_options.noop.enable = true;
    }
    let instance = wgpu::Instance::new(descriptor);
    let Ok(adapter) =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
    else {
        eprintln!("no GPU adapter available");
        return;
    };
    println!("adapter: {}", adapter.get_info().name);
    let Ok((device, queue)) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
    else {
        eprintln!("no device");
        return;
    };
    let poll_device = device.clone();

    let mut globe = Globe::new(
        device,
        queue,
        TileProvider::new(offline(TileSource::open_street_map())),
        (1920, 1080),
    );
    globe.set_terrain_source(offline(TileSource::terrain_terrarium()));
    globe.set_terrain_enabled(std::env::var_os("TERRA_BENCH_FLAT").is_none());
    globe.camera.target_lon_deg = 10.0;
    globe.camera.target_lat_deg = 46.5;
    globe.camera.distance = 3_000_000.0;
    globe.camera.heading = 0.0;
    globe.camera.pitch = 80f64.to_radians();

    let phases = [
        Phase {
            name: "zoom in",
            frames: 240,
            step: |globe, _| globe.camera.distance = (globe.camera.distance * 0.975).max(4_000.0),
        },
        Phase {
            name: "tilt to the horizon",
            frames: 90,
            step: |globe, _| {
                globe.camera.pitch = (globe.camera.pitch - 0.75f64.to_radians()).max(0.2);
            },
        },
        Phase {
            name: "fly low",
            frames: 240,
            step: |globe, _| globe.move_view(60.0, 10.0),
        },
        Phase {
            name: "look around",
            frames: 180,
            step: |globe, _| globe.rotate_by(1f64.to_radians()),
        },
        Phase {
            name: "hold still",
            frames: 120,
            step: |_, _| {},
        },
    ];

    let image = RgbaImage::from_pixel(
        TILE_TEXTURE_SIZE,
        TILE_TEXTURE_SIZE,
        image::Rgba([120, 140, 90, 255]),
    );
    println!(
        "{:<20} {:>7} {:>7} {:>7} {:>7} {:>6} {:>7}",
        "phase", "mean ms", "p95 ms", "max ms", "builds", "tiles", "meshes"
    );
    for phase in phases {
        let mut times = Vec::with_capacity(phase.frames as usize);
        let (mut builds, mut tiles, mut meshes) = (0, 0, 0);
        for frame in 0..phase.frames {
            (phase.step)(&mut globe, f64::from(frame));
            globe.settle_camera();
            let stats = globe.render(0.0);
            times.push(f64::from(stats.prepare_micros) / 1000.0);
            builds += stats.meshes_built;
            tiles = stats.tiles_drawn;
            meshes = stats.meshes_resident;
            let _ = poll_device.poll(wgpu::PollType::Poll);

            let selection: Vec<_> = globe.renderer.last_selection().to_vec();
            let mut textures = Vec::new();
            for tile in &selection {
                let elevation = elevation_tile_for(tile.id);
                if !globe.renderer.terrain().cache().contains(elevation) {
                    globe.inject_terrain_tile(elevation, TerrainTile::synthetic(elevation, relief));
                }
                if !globe.renderer.has_texture(tile.id) {
                    textures.push(FetchedTile {
                        id: tile.id,
                        result: Ok(image.clone()),
                    });
                }
            }
            globe.renderer.ingest(textures);
        }
        times.sort_by(f64::total_cmp);
        let mean = times.iter().sum::<f64>() / times.len() as f64;
        let p95 = times[(times.len() * 95 / 100).min(times.len() - 1)];
        let max = times.last().copied().unwrap_or(0.0);
        println!(
            "{:<20} {mean:>7.2} {p95:>7.2} {max:>7.2} {builds:>7} {tiles:>6} {meshes:>7}",
            phase.name
        );
    }
}
