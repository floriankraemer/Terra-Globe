//! High-level globe facade used by the application layer.
//!
//! Bundles the camera, renderer, imagery pipeline and tile selection so the UI
//! only has to forward input and draw the resulting texture.

use glam::DVec3;

use crate::camera::{Camera, CameraPose, ScreenPoint};
use crate::flat::{FlatCamera, SURFACE_Z, SceneMode, lon_lat_to_plane, plane_to_lon_lat};
use crate::geo::{
    Geodetic, TileId, ecef_to_geodetic, geodetic_to_ecef, is_point_visible, ray_ellipsoid_point,
};
use crate::imagery::{TileFetcher, TileProvider};
use crate::picking::PickHit;
use crate::renderer::{GlobeRenderer, RenderStats};
use crate::scene::Scene;
use crate::tiles::SelectionOptions;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlobePick {
    pub lon_deg: f64,
    pub lat_deg: f64,
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
            crate::picking::pick(&self.scene, &self.camera, self.viewport, pixel)
        }
    }

    /// Flies to a geometry, sizing the distance from its extent.
    pub fn fly_to_geometry(&mut self, geometry: &terra_core::PlacemarkGeometry, now_seconds: f64) {
        let center = terra_core::geometry_center(geometry);
        let extent = terra_core::geometry_extent_meters(geometry);
        let distance = (extent * 3.0).clamp(1_500.0, 20_000_000.0);
        self.fly_to(center.lon, center.lat, distance, now_seconds);
    }

    /// True while imagery is still being fetched, so the UI can keep repainting
    /// and pick up the tiles as they arrive instead of idling at a low frame rate.
    pub fn is_loading_imagery(&self) -> bool {
        let stats = self.renderer.stats();
        stats.requests_in_flight > 0 || stats.requests_queued > 0
    }

    pub fn render(&mut self, now_seconds: f64) -> RenderStats {
        if !self.mode.is_flat() {
            self.camera.update(now_seconds);
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
        let (lon_deg, lat_deg) = if self.mode.is_flat() {
            let plane = self.flat.pick_plane(pixel, self.viewport)?;
            let point = plane_to_lon_lat(plane);
            (point.lon, point.lat)
        } else {
            let (origin, direction) = self.camera.ray_at(pixel, self.viewport);
            let hit = ray_ellipsoid_point(origin, direction)?;
            let geodetic = ecef_to_geodetic(hit);
            (geodetic.lon_deg(), geodetic.lat_deg())
        };
        Some(GlobePick {
            lon_deg,
            lat_deg,
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

    /// Drag rotation: orbit in 3D and Columbus, ignored in 2D (which pans).
    pub fn orbit(&mut self, delta_x: f64, delta_y: f64) {
        match self.mode {
            SceneMode::Scene3D => self.camera.orbit(delta_x, delta_y),
            SceneMode::Columbus => self.flat.orbit(delta_x, delta_y),
            SceneMode::Scene2D => {}
        }
    }

    /// Drag pan for the flat modes; the 3D globe pans by orbiting.
    pub fn pan_pixels(&mut self, delta: (f64, f64)) {
        if self.mode.is_flat() {
            self.flat.pan_pixels(delta, self.viewport);
        }
    }

    /// Zooms toward the cursor: the picked surface point moves towards the
    /// centre of the screen while the distance shrinks, so the feature under the
    /// pointer stays under the pointer.
    pub fn zoom_at(&mut self, pixel: (f64, f64), factor: f64) {
        if self.mode.is_flat() {
            self.flat.zoom_at(pixel, self.viewport, factor);
            return;
        }
        let Some(pick) = self.pick(pixel) else {
            self.camera.zoom(factor);
            return;
        };
        let weight = if factor < 1.0 { 0.35 } else { 0.15 };
        // Interpolating the geographic target linearly is visually fine at the
        // distances zoom-at-cursor is used; `fly_to` handles long hops.
        self.camera.target_lon_deg += (pick.lon_deg - self.camera.target_lon_deg) * weight;
        self.camera.target_lat_deg += (pick.lat_deg - self.camera.target_lat_deg) * weight;
        self.camera.zoom(factor);
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
