//! High-level globe facade used by the application layer.
//!
//! Bundles the camera, renderer, imagery pipeline and tile selection so the UI
//! only has to forward input and draw the resulting texture.

use std::sync::Arc;

use glam::DVec3;

use crate::camera::{Camera, CameraPose, ScreenPoint};
use crate::flat::{FlatCamera, SURFACE_Z, SceneMode, lon_lat_to_plane, plane_to_lon_lat};
use crate::geo::{
    Geodetic, GroundFn, TileId, ecef_to_geodetic, geodetic_to_ecef, is_point_visible,
    ray_ellipsoid_point, ray_terrain,
};
use crate::imagery::{ImageOverlay, TileCache, TileFetcher, TileOverlay, TileProvider};
use crate::picking::PickHit;
use crate::renderer::{GlobeRenderer, RenderStats};
use crate::scene::Scene;
use crate::tiles::SelectionOptions;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlobePick {
    pub lon_deg: f64,
    pub lat_deg: f64,
    /// Height of the picked surface above the ellipsoid, in meters: terrain
    /// height when terrain is on, otherwise 0.
    pub height_m: f64,
    /// Where the pick landed on screen, in pixels.
    pub screen: ScreenPoint,
}

pub struct Globe {
    pub camera: Camera,
    pub flat: FlatCamera,
    pub mode: SceneMode,
    pub renderer: GlobeRenderer,
    pub provider: TileProvider,
    pub fetcher: TileFetcher,
    viewport: (u32, u32),
    scene: Scene,
    scene_revision: u64,
    tile_overlays: Vec<TileOverlay>,
}

impl Globe {
    pub fn new(
        device: wgpu::Device,
        queue: wgpu::Queue,
        provider: TileProvider,
        viewport: (u32, u32),
    ) -> Self {
        let mut renderer = GlobeRenderer::new(device, queue);
        renderer.resize(viewport.0, viewport.1);
        let camera = Camera::default();
        let flat = FlatCamera::from_camera(&camera, SceneMode::Scene3D);
        Self {
            camera,
            flat,
            mode: SceneMode::Scene3D,
            renderer,
            provider,
            fetcher: TileFetcher::new(),
            viewport,
            scene: Scene::default(),
            scene_revision: 0,
            tile_overlays: Vec::new(),
        }
    }

    pub fn viewport(&self) -> (u32, u32) {
        self.viewport
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.viewport = (width.max(1), height.max(1));
        self.renderer.resize(width.max(1), height.max(1));
    }

    pub fn set_provider(&mut self, provider: TileProvider) {
        if provider.source.id != self.provider.source.id {
            // Cached tiles belong to the previous provider and tile caches are
            // keyed by tile address, so the imagery has to be dropped or the new
            // layer would show the old one's tiles.
            self.renderer.clear_caches();
        }
        self.provider = provider;
    }

    /// Serves tiles that have already been downloaded from a persistent store,
    /// so revisiting an area does not hit the network again.
    pub fn set_tile_cache(&mut self, cache: Arc<dyn TileCache>) {
        self.fetcher.set_cache(Arc::clone(&cache));
        self.renderer.terrain_mut().set_cache(Arc::clone(&cache));
        self.renderer.set_overlay_cache(cache);
    }

    /// Turns the 3D terrain on or off. Off keeps every tile, line and marker
    /// on the ellipsoid; the 2D and Columbus modes never use terrain.
    pub fn set_terrain_enabled(&mut self, enabled: bool) {
        self.renderer.terrain_mut().set_enabled(enabled);
        self.settle_camera();
    }

    pub fn terrain_enabled(&self) -> bool {
        self.renderer.terrain().enabled()
    }

    /// Whether the view is displaced by terrain right now.
    pub fn terrain_active(&self) -> bool {
        self.terrain_enabled() && !self.mode.is_flat()
    }

    /// Changes where elevation tiles come from (tests point it at a local
    /// server or an address that never answers).
    pub fn set_terrain_source(&mut self, source: terra_providers::TileSource) {
        self.renderer.terrain_mut().set_source(source);
    }

    /// Supplies one elevation tile directly, without the network. For tests
    /// and tools that bring their own heights.
    pub fn inject_terrain_tile(&mut self, id: TileId, tile: crate::terrain::TerrainTile) {
        self.renderer.terrain_mut().cache_mut().insert(id, tile);
        self.settle_camera();
    }

    /// Height of the drawn ground at a point, in meters above the ellipsoid.
    /// `None` when terrain is off, in a flat mode, or not loaded there; callers
    /// then use the ellipsoid.
    pub fn ground_height(&self, lon_deg: f64, lat_deg: f64) -> Option<f64> {
        self.terrain_active()
            .then(|| self.renderer.terrain().ground(lon_deg, lat_deg))
            .flatten()
    }

    /// Runs `f` with the camera and the ground lookup, which is `None` unless
    /// terrain is active (so the camera takes its ellipsoid paths).
    fn with_ground<R>(&mut self, f: impl FnOnce(&mut Camera, Option<&GroundFn<'_>>) -> R) -> R {
        let terrain = self.renderer.terrain();
        let active = terrain.enabled() && !self.mode.is_flat();
        let ground = |lon: f64, lat: f64| terrain.ground(lon, lat);
        f(&mut self.camera, active.then_some(&ground))
    }

    /// Puts the camera target on the ground and keeps the eye above it. Done
    /// after every camera input and every frame, because terrain loads and
    /// moves under a camera that did not.
    pub fn settle_camera(&mut self) {
        if self.mode.is_flat() {
            return;
        }
        self.with_ground(|camera, ground| {
            camera.target_height = ground
                .and_then(|ground| ground(camera.target_lon_deg, camera.target_lat_deg))
                .unwrap_or(0.0);
            camera.ensure_above_surface_on(ground);
        });
    }

    /// Transparent tile layers over the basemap, bottom to top. Layers that are
    /// still present keep their imagery, so toggling one or changing its
    /// opacity does not download anything again. Switching the basemap leaves
    /// them alone.
    pub fn set_tile_overlays(&mut self, layers: Vec<TileOverlay>) {
        self.renderer.set_tile_overlays(&layers);
        self.tile_overlays = layers;
    }

    /// Georeferenced pictures (KML `GroundOverlay`) drawn above the tile
    /// layers, in the given order.
    pub fn set_image_overlays(&mut self, overlays: Vec<ImageOverlay>) {
        self.renderer.set_image_overlays(&overlays);
    }

    /// Attribution for everything currently shown: the basemap, then each
    /// active tile layer, then the terrain model when it is in use, without
    /// duplicates. Hidden (zero opacity) layers are
    /// left out.
    pub fn credits(&self) -> Vec<String> {
        let mut credits: Vec<String> = Vec::new();
        let layers = self
            .tile_overlays
            .iter()
            .filter(|layer| layer.opacity > 0.0)
            .map(|layer| layer.source.credit.as_str());
        let terrain = self
            .terrain_active()
            .then(|| self.renderer.terrain().credit());
        for credit in std::iter::once(self.provider.credit())
            .chain(layers)
            .chain(terrain)
        {
            if !credit.trim().is_empty() && !credits.iter().any(|known| known == credit) {
                credits.push(credit.to_string());
            }
        }
        credits
    }

    pub fn set_selection_options(&mut self, options: SelectionOptions) {
        self.renderer.set_selection_options(options);
    }

    /// Replaces overlay geometry; pass a revision that changes whenever the
    /// library or the selection changes.
    pub fn set_scene(&mut self, revision: u64, scene: Scene) {
        self.renderer.set_scene(revision, &scene, self.mode);
        self.scene = scene;
        self.scene_revision = revision;
    }

    pub fn mode(&self) -> SceneMode {
        self.mode
    }

    /// Switches between the globe and the two flat map modes, keeping the
    /// geographic centre and an approximate zoom.
    pub fn set_mode(&mut self, mode: SceneMode) {
        if mode == self.mode {
            return;
        }
        match mode {
            SceneMode::Scene3D => {
                let mut pose = self.flat.to_pose(self.mode);
                pose.pitch = pose.pitch.clamp(10f64.to_radians(), 89.5f64.to_radians());
                pose.distance = pose.distance.clamp(8.0, 40_000_000.0);
                self.camera.fly_to(pose, 0.0, 0.0);
            }
            SceneMode::Scene2D => {
                self.flat = FlatCamera::from_camera(&self.camera, mode);
                self.flat.pitch = std::f64::consts::FRAC_PI_2;
                // The orthographic path has no rotation, so a rotated 3D view
                // would make picking and rendering disagree.
                self.flat.heading = 0.0;
            }
            SceneMode::Columbus => {
                self.flat = FlatCamera::from_camera(&self.camera, mode);
                self.flat.pitch = 50f64.to_radians();
            }
        }
        self.mode = mode;
        self.renderer
            .set_scene(self.scene_revision, &self.scene, mode);
    }

    /// Distance used for label thresholds: camera distance (3D) or view height.
    pub fn view_distance(&self) -> f64 {
        if self.mode.is_flat() {
            self.flat.distance
        } else {
            self.camera.distance
        }
    }

    /// Pose of the active mode, for settings persistence.
    pub fn pose(&self) -> CameraPose {
        if self.mode.is_flat() {
            self.flat.to_pose(self.mode)
        } else {
            self.camera.pose()
        }
    }

    pub fn apply_pose(&mut self, pose: CameraPose) {
        if self.mode.is_flat() {
            self.flat = FlatCamera::from_pose(&pose).with_mode(self.mode);
        } else {
            self.camera.apply_pose(pose);
        }
    }

    pub fn scene(&self) -> &Scene {
        &self.scene
    }

    pub fn pick_placemark(&self, pixel: (f64, f64)) -> Option<PickHit> {
        if self.mode.is_flat() {
            crate::picking::pick_flat(&self.scene, &self.flat, self.viewport, pixel)
        } else {
            let terrain = self.renderer.terrain();
            let ground = |lon: f64, lat: f64| {
                if terrain.enabled() {
                    terrain.ground(lon, lat)
                } else {
                    None
                }
            };
            crate::picking::pick(&self.scene, &self.camera, self.viewport, pixel, &ground)
        }
    }

    /// Flies to a geometry, sizing the distance from its extent.
    pub fn fly_to_geometry(&mut self, geometry: &terra_core::PlacemarkGeometry, now_seconds: f64) {
        let center = terra_core::geometry_center(geometry);
        let extent = terra_core::geometry_extent_meters(geometry);
        let distance = (extent * 3.0).clamp(1_500.0, 20_000_000.0);
        self.fly_to(center.lon, center.lat, distance, now_seconds);
    }

    /// True while imagery is still being fetched or an overlay picture is still
    /// being prepared, so the UI can keep repainting and pick them up as they
    /// arrive instead of idling at a low frame rate.
    pub fn is_loading_imagery(&self) -> bool {
        let stats = self.renderer.stats();
        stats.requests_in_flight > 0
            || stats.requests_queued > 0
            || self.renderer.has_pending_images()
    }

    pub fn render(&mut self, now_seconds: f64) -> RenderStats {
        if !self.mode.is_flat() {
            self.camera.update(now_seconds);
            self.settle_camera();
        }
        self.renderer.render(
            self.mode,
            &self.camera,
            &self.flat,
            &self.provider,
            &mut self.fetcher,
        )
    }

    /// Screen pixels to a point on the globe (3D) or the map plane (flat).
    pub fn pick(&self, pixel: (f64, f64)) -> Option<GlobePick> {
        let (lon_deg, lat_deg, height_m) = if self.mode.is_flat() {
            let plane = self.flat.pick_plane(pixel, self.viewport)?;
            let point = plane_to_lon_lat(plane);
            (point.lon, point.lat, 0.0)
        } else {
            let (origin, direction) = self.camera.ray_at(pixel, self.viewport);
            let terrain = self.renderer.terrain();
            let hit = if terrain.enabled() {
                ray_terrain(origin, direction, &|lon, lat| terrain.ground(lon, lat))?
            } else {
                ray_ellipsoid_point(origin, direction)?
            };
            let geodetic = ecef_to_geodetic(hit);
            let height = if terrain.enabled() {
                geodetic.height
            } else {
                0.0
            };
            (geodetic.lon_deg(), geodetic.lat_deg(), height)
        };
        Some(GlobePick {
            lon_deg,
            lat_deg,
            height_m,
            screen: ScreenPoint {
                x: pixel.0,
                y: pixel.1,
                depth: 0.0,
            },
        })
    }

    /// Projects a geographic point, returning `None` when it is behind the
    /// globe or off screen. This is what labels and markers use for placement.
    pub fn project(&self, point: Geodetic) -> Option<ScreenPoint> {
        if self.mode.is_flat() {
            let geographic = terra_core::GeoPoint::new(point.lon_deg(), point.lat_deg()).ok()?;
            let plane = lon_lat_to_plane(&geographic);
            return self
                .flat
                .project(glam::DVec3::new(plane.x, plane.y, SURFACE_Z), self.viewport);
        }
        let ecef = geodetic_to_ecef(point);
        if !is_point_visible(self.camera.eye_ecef(), ecef) {
            return None;
        }
        self.camera.project(ecef, self.viewport)
    }

    pub fn fly_to(&mut self, lon_deg: f64, lat_deg: f64, distance: f64, now_seconds: f64) {
        if self.mode.is_flat() {
            if let Ok(point) = terra_core::GeoPoint::new(lon_deg, lat_deg) {
                self.flat.fly_to(&point, distance);
            }
            return;
        }
        let pose = CameraPose {
            lon_deg,
            lat_deg,
            distance: distance.clamp(8.0, 40_000_000.0),
            heading: self.camera.heading,
            pitch: self.camera.pitch,
        };
        self.camera.fly_to(pose, 1.2, now_seconds);
    }

    /// Shift-drag / right-drag: rotate the view and tilt it. Ignored in the
    /// orthographic 2D view, which is north-up by construction.
    pub fn orbit(&mut self, delta_x: f64, delta_y: f64) {
        match self.mode {
            SceneMode::Scene3D => {
                self.camera.orbit(delta_x, delta_y);
                self.settle_camera();
            }
            SceneMode::Columbus => self.flat.orbit(delta_x, delta_y),
            SceneMode::Scene2D => {}
        }
    }

    /// Rotates the view. Positive radians turn the imagery to the right.
    pub fn rotate_by(&mut self, radians: f64) {
        match self.mode {
            SceneMode::Scene3D => self.camera.rotate_by(radians),
            SceneMode::Columbus => self.flat.rotate_by(radians),
            SceneMode::Scene2D => {}
        }
    }

    /// Google Earth's look-around: turns the view about the fixed eye.
    /// Only meaningful in 3D; the flat views have no free eye to turn.
    pub fn look_by(&mut self, d_heading: f64, d_pitch: f64) {
        if self.mode == SceneMode::Scene3D {
            self.with_ground(|camera, ground| camera.look_by_on(d_heading, d_pitch, ground));
            self.settle_camera();
        }
    }

    /// Tilts the view. Positive radians raise the eye towards top-down.
    pub fn tilt_by(&mut self, radians: f64) {
        match self.mode {
            SceneMode::Scene3D => {
                self.camera.tilt_by(radians);
                self.settle_camera();
            }
            SceneMode::Columbus => self.flat.tilt_by(radians),
            SceneMode::Scene2D => {}
        }
    }

    /// Drags the map: the ground under `pixel` follows the cursor, in every
    /// mode. 3D uses the pixel to work out the local ground scale.
    pub fn pan_pixels(&mut self, pixel: (f64, f64), delta: (f64, f64)) {
        if self.mode.is_flat() {
            self.flat.pan_pixels(delta, self.viewport);
        } else {
            let viewport = self.viewport;
            self.with_ground(|camera, ground| camera.pan_pixels_on(pixel, viewport, delta, ground));
            self.settle_camera();
        }
    }

    /// Moves the view along the screen axes, in metres on the ground. This is
    /// what the arrow keys drive: `forward` is towards the top of the screen,
    /// `right` towards the right edge, both relative to the current heading.
    pub fn move_view(&mut self, forward_m: f64, right_m: f64) {
        if self.mode.is_flat() {
            self.flat.pan_view(forward_m, right_m);
        } else {
            self.camera.pan_view(forward_m, right_m);
            self.settle_camera();
        }
    }

    /// Zooms towards the cursor, Google Earth style: the point under the
    /// pointer stays put and the view does not rotate.
    pub fn zoom_at(&mut self, pixel: (f64, f64), factor: f64) {
        if self.mode.is_flat() {
            self.flat.zoom_at(pixel, self.viewport, factor);
        } else {
            let viewport = self.viewport;
            self.with_ground(|camera, ground| camera.dolly_at_on(pixel, viewport, factor, ground));
            self.settle_camera();
        }
    }

    /// Zooms towards the middle of the screen.
    pub fn zoom_by(&mut self, factor: f64) {
        let centre = (
            f64::from(self.viewport.0) * 0.5,
            f64::from(self.viewport.1) * 0.5,
        );
        self.zoom_at(centre, factor);
    }

    /// Google Earth's `n`: put north at the top.
    pub fn set_north_up(&mut self) {
        match self.mode {
            SceneMode::Scene3D => self.camera.set_heading(0.0),
            SceneMode::Columbus => self.flat.set_heading(0.0),
            SceneMode::Scene2D => {}
        }
    }

    /// Google Earth's `u`: look straight down.
    pub fn set_top_down(&mut self) {
        match self.mode {
            SceneMode::Scene3D => {
                self.camera.set_pitch(std::f64::consts::FRAC_PI_2);
                self.settle_camera();
            }
            SceneMode::Columbus => self.flat.tilt_by(std::f64::consts::FRAC_PI_2),
            SceneMode::Scene2D => {}
        }
    }

    /// Google Earth's `r`: north-up and top-down in one step.
    pub fn reset_view(&mut self) {
        self.set_north_up();
        self.set_top_down();
    }

    /// Space bar: cancel a running camera animation.
    pub fn stop_motion(&mut self) {
        self.camera.stop();
    }

    pub fn eye_ecef(&self) -> DVec3 {
        self.camera.eye_ecef()
    }

    /// Newly created tiles have no texture until their request completes.
    pub fn is_tile_ready(&self, id: TileId) -> bool {
        self.renderer.has_texture(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_gpu_globe() -> Option<Globe> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
                .ok()?;
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).ok()?;
        Some(Globe::new(
            device,
            queue,
            TileProvider::new(terra_providers::TileSource::open_street_map()),
            (640, 400),
        ))
    }

    #[test]
    fn picking_the_screen_centre_returns_the_camera_target() {
        let Some(globe) = no_gpu_globe() else {
            eprintln!("no GPU adapter available; skipping pick test");
            return;
        };
        let pick = globe
            .pick((320.0, 200.0))
            .expect("centre of the screen must hit the globe");
        assert!((pick.lon_deg - globe.camera.target_lon_deg).abs() < 0.5);
        assert!((pick.lat_deg - globe.camera.target_lat_deg).abs() < 0.5);
    }

    #[test]
    fn projecting_the_target_returns_the_centre() {
        let Some(globe) = no_gpu_globe() else {
            return;
        };
        let screen = globe
            .project(globe.camera.target_geodetic())
            .expect("target is visible");
        assert!((screen.x - 320.0).abs() < 1.0);
        assert!((screen.y - 200.0).abs() < 1.0);
    }
}
