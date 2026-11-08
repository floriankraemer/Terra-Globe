//! Flat map projections: 2D (orthographic) and Columbus (perspective over the
//! same plane).
//!
//! The plane is Web-Mercator in meters: the world is `2*pi*R` wide, `x` grows
//! east and `y` grows north. Geometry is placed at `SURFACE_Z` so overlays sort
//! in front of the imagery without needing a separate depth strategy.

use glam::{DVec3, Mat4, Vec2, Vec4};
use terra_core::GeoPoint;

use crate::geo::TileId;

use crate::camera::{Camera, CameraPose, ScreenPoint};
use crate::geo::{MERCATOR_MAX_LAT_DEG, WGS84_A};

/// Width of the Mercator world at the equator, in meters.
pub const WORLD_WIDTH_METERS: f64 = 2.0 * std::f64::consts::PI * WGS84_A;
/// The Mercator plane is square: `v` spans 0..1 from +85.05 to -85.05 degrees.
pub const WORLD_HEIGHT_METERS: f64 = WORLD_WIDTH_METERS;
/// Overlays sit this far above the imagery plane.
pub const SURFACE_Z: f64 = 50.0;
/// Camera height used by the orthographic projection.
const ORTHO_EYE_HEIGHT: f64 = 1_000_000.0;
const MIN_SCALE: f64 = 200.0;
const MAX_SCALE: f64 = WORLD_WIDTH_METERS * 4.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SceneMode {
    #[default]
    Scene3D,
    Scene2D,
    Columbus,
}

impl SceneMode {
    pub fn is_flat(self) -> bool {
        !matches!(self, Self::Scene3D)
    }

    pub fn label_key(self) -> &'static str {
        match self {
            Self::Scene3D => "sceneMode.scene3d",
            Self::Scene2D => "sceneMode.scene2d",
            Self::Columbus => "sceneMode.columbus",
        }
    }
}

/// Position on the flat map plane in meters, `x` east and `y` north.
pub fn lon_lat_to_plane(point: &GeoPoint) -> DVec3 {
    let lon = point.lon.clamp(-180.0, 180.0);
    let lat = point.lat.clamp(-MERCATOR_MAX_LAT_DEG, MERCATOR_MAX_LAT_DEG);
    let x = lon / 360.0 * WORLD_WIDTH_METERS;
    let y = (std::f64::consts::PI / 4.0 + lat.to_radians() / 2.0)
        .tan()
        .ln()
        / (2.0 * std::f64::consts::PI)
        * WORLD_WIDTH_METERS;
    DVec3::new(x, y, 0.0)
}

pub fn plane_to_lon_lat(point: DVec3) -> GeoPoint {
    let lon = point.x / WORLD_WIDTH_METERS * 360.0;
    let lat = (point.y / WORLD_WIDTH_METERS * 2.0 * std::f64::consts::PI)
        .sinh()
        .atan()
        .to_degrees();
    GeoPoint::new(lon.clamp(-180.0, 180.0), lat.clamp(-90.0, 90.0)).unwrap_or(GeoPoint {
        lon: 0.0,
        lat: 0.0,
        altitude: None,
        altitude_mode: None,
    })
}

/// Plane-space rectangle covered by one Mercator tile.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlaneRect {
    pub min: Vec2,
    pub max: Vec2,
}

impl PlaneRect {
    pub fn from_tile(id: TileId) -> Self {
        let count = 2f64.powi(i32::from(id.z));
        let u0 = f64::from(id.x) / count;
        let u1 = f64::from(id.x + 1) / count;
        let v0 = f64::from(id.y) / count;
        let v1 = f64::from(id.y + 1) / count;
        Self {
            min: Vec2::new(
                (u0 - 0.5) as f32 * WORLD_WIDTH_METERS as f32,
                (0.5 - v1) as f32 * WORLD_WIDTH_METERS as f32,
            ),
            max: Vec2::new(
                (u1 - 0.5) as f32 * WORLD_WIDTH_METERS as f32,
                (0.5 - v0) as f32 * WORLD_WIDTH_METERS as f32,
            ),
        }
    }

    pub fn center(&self) -> Vec2 {
        (self.min + self.max) * 0.5
    }

    pub fn width(&self) -> f32 {
        self.max.x - self.min.x
    }

    pub fn intersects(&self, other: &PlaneRect) -> bool {
        self.min.x <= other.max.x
            && self.max.x >= other.min.x
            && self.min.y <= other.max.y
            && self.max.y >= other.min.y
    }

    pub fn contains(&self, point: Vec2) -> bool {
        point.x >= self.min.x
            && point.x <= self.max.x
            && point.y >= self.min.y
            && point.y <= self.max.y
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlatCamera {
    /// Plane coordinate at the centre of the view.
    pub center: Vec2,
    /// Rotation around the target; 0 = north up.
    pub heading: f64,
    /// Tilt of the eye above the horizon (Columbus); 90 degrees in 2D.
    pub pitch: f64,
    /// View height (2D) or eye distance (Columbus), in meters.
    pub distance: f64,
}

impl Default for FlatCamera {
    fn default() -> Self {
        let center = lon_lat_to_plane(&GeoPoint {
            lon: 10.0,
            lat: 35.0,
            altitude: None,
            altitude_mode: None,
        });
        Self {
            center: Vec2::new(center.x as f32, center.y as f32),
            heading: 0.0,
            pitch: std::f64::consts::FRAC_PI_2,
            distance: 20_000_000.0,
        }
    }
}

impl FlatCamera {
    pub fn from_pose(pose: &CameraPose) -> Self {
        let plane = lon_lat_to_plane(&GeoPoint {
            lon: pose.lon_deg,
            lat: pose.lat_deg,
            altitude: None,
            altitude_mode: None,
        });
        Self {
            center: Vec2::new(plane.x as f32, plane.y as f32),
            heading: pose.heading,
            pitch: pose.pitch,
            distance: pose.distance,
        }
    }

    pub fn to_pose(&self, mode: SceneMode) -> CameraPose {
        let point = plane_to_lon_lat(DVec3::new(
            f64::from(self.center.x),
            f64::from(self.center.y),
            0.0,
        ));
        CameraPose {
            lon_deg: point.lon,
            lat_deg: point.lat,
            distance: self.distance,
            heading: self.heading,
            pitch: if mode == SceneMode::Scene2D {
                std::f64::consts::FRAC_PI_2
            } else {
                self.pitch
            },
        }
    }

    pub fn plane_center_point(&self) -> GeoPoint {
        plane_to_lon_lat(DVec3::new(
            f64::from(self.center.x),
            f64::from(self.center.y),
            0.0,
        ))
    }

    pub fn is_orthographic(&self) -> bool {
        self.pitch >= std::f64::consts::FRAC_PI_2
    }

    pub fn with_mode(mut self, mode: SceneMode) -> Self {
        match mode {
            SceneMode::Scene3D => self,
            SceneMode::Scene2D => {
                self.pitch = std::f64::consts::FRAC_PI_2;
                self
            }
            SceneMode::Columbus => {
                if self.pitch >= std::f64::consts::FRAC_PI_2 {
                    self.pitch = 50f64.to_radians();
                }
                self
            }
        }
    }

    /// Visible plane rectangle for culling and tile selection. Perspective views
    /// that look at the horizon return a generously sized rectangle instead of
    /// failing.
    pub fn visible_rect(&self, viewport: (u32, u32)) -> PlaneRect {
        let aspect = f64::from(viewport.0.max(1)) / f64::from(viewport.1.max(1));
        let center = DVec3::new(f64::from(self.center.x), f64::from(self.center.y), 0.0);
        if self.is_orthographic() {
            let half_height = self.distance / 2.0;
            let half_width = half_height * aspect;
            let (sin, cos) = self.heading.sin_cos();
            let right = DVec3::new(cos, -sin, 0.0) * half_width;
            let up = DVec3::new(sin, cos, 0.0) * half_height;
            let corners = [
                center - right - up,
                center + right - up,
                center + right + up,
                center - right + up,
            ];
            return bounds(&corners);
        }

        let mut corners = Vec::new();
        for ndc in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            if let Some(point) = self.ray_plane_point(ndc, aspect) {
                corners.push(point);
            }
        }
        if corners.len() < 2 {
            // Looking at or above the horizon: use a rectangle proportional to the
            // distance so imagery for the far plane is still requested.
            let half = self.distance * 4.0;
            return PlaneRect {
                min: Vec2::new(self.center.x - half as f32, self.center.y - half as f32),
                max: Vec2::new(self.center.x + half as f32, self.center.y + half as f32),
            };
        }
        bounds(&corners)
    }

    /// NDC position (x, y) of a plane point, plus clip w.
    pub fn project_ndc(&self, point: DVec3, aspect: f64) -> Option<(f64, f64)> {
        let relative = (point - self.eye_plane()).as_vec3();
        let clip = self.view_projection(aspect) * relative.extend(1.0);
        if clip.w <= 0.0 {
            return None;
        }
        Some((f64::from(clip.x / clip.w), f64::from(clip.y / clip.w)))
    }

    pub fn project(&self, point: DVec3, viewport: (u32, u32)) -> Option<ScreenPoint> {
        let aspect = f64::from(viewport.0.max(1)) / f64::from(viewport.1.max(1));
        let relative = (point - self.eye_plane()).as_vec3();
        let clip = self.view_projection(aspect) * relative.extend(1.0);
        if clip.w <= 0.0 {
            return None;
        }
        let ndc = clip.truncate() / clip.w;
        if !(-1.0..=1.0).contains(&ndc.x) || !(-1.0..=1.0).contains(&ndc.y) {
            return None;
        }
        Some(ScreenPoint {
            x: (f64::from(ndc.x) * 0.5 + 0.5) * f64::from(viewport.0),
            y: (1.0 - (f64::from(ndc.y) * 0.5 + 0.5)) * f64::from(viewport.1),
            depth: f64::from(ndc.z),
        })
    }

    /// Rotation-only view, because every position handed to the GPU is already
    /// eye-relative (the same convention as the globe renderer).
    fn view_rotation(&self) -> Mat4 {
        if self.is_orthographic() {
            Mat4::IDENTITY
        } else {
            let (sin_heading, cos_heading) = self.heading.sin_cos();
            let (sin_pitch, cos_pitch) = self.pitch.sin_cos();
            let forward = DVec3::new(
                -sin_heading * cos_pitch,
                cos_heading * cos_pitch,
                -sin_pitch,
            );
            glam::camera::rh::view::look_to_mat4(glam::Vec3::ZERO, forward.as_vec3(), glam::Vec3::Z)
        }
    }

    pub fn view_projection(&self, aspect: f64) -> Mat4 {
        let aspect = if aspect.is_finite() && aspect > 0.0 {
            aspect
        } else {
            1.0
        };
        if self.is_orthographic() {
            let half_height = self.distance / 2.0;
            let half_width = half_height * aspect;
            let near = 1.0;
            let far = ORTHO_EYE_HEIGHT * 2.0;
            // Reverse-Z orthographic: the plane maps near 0.5, overlays slightly
            // higher (closer), and the far field to 0.
            // View-space z is negative; reverse-Z maps the near plane to 1 and
            // the far plane to 0, so the plane (halfway down the range) lands at
            // ~0.5 and overlays slightly above it.
            let z_scale = 1.0 / (far - near);
            let z_offset = 1.0 + near / (far - near);
            Mat4::from_cols(
                Vec4::new((1.0 / half_width) as f32, 0.0, 0.0, 0.0),
                Vec4::new(0.0, (1.0 / half_height) as f32, 0.0, 0.0),
                Vec4::new(0.0, 0.0, z_scale as f32, 0.0),
                Vec4::new(0.0, 0.0, z_offset as f32, 1.0),
            ) * self.view_rotation()
        } else {
            let f = 1.0 / (std::f64::consts::FRAC_PI_4 / 2.0).tan();
            let near = 10.0;
            let projection = Mat4::from_cols(
                Vec4::new((f / aspect) as f32, 0.0, 0.0, 0.0),
                Vec4::new(0.0, f as f32, 0.0, 0.0),
                Vec4::new(0.0, 0.0, 0.0, -1.0),
                Vec4::new(0.0, 0.0, near as f32, 0.0),
            );
            projection * self.view_rotation()
        }
    }

    /// Ray through an NDC position, in plane space.
    pub fn ray_ndc(&self, ndc: (f64, f64), aspect: f64) -> (DVec3, DVec3) {
        if self.is_orthographic() {
            let half_height = self.distance / 2.0;
            let half_width = half_height * aspect;
            let (sin, cos) = self.heading.sin_cos();
            let offset_x = ndc.0 * half_width;
            let offset_y = ndc.1 * half_height;
            let world_x = offset_x * cos - offset_y * sin;
            let world_y = offset_x * sin + offset_y * cos;
            (
                DVec3::new(
                    f64::from(self.center.x) + world_x,
                    f64::from(self.center.y) + world_y,
                    ORTHO_EYE_HEIGHT,
                ),
                DVec3::new(0.0, 0.0, -1.0),
            )
        } else {
            let tan_half = (std::f64::consts::FRAC_PI_4 / 2.0).tan();
            let inverse = self.view_rotation().inverse();
            let view_direction = glam::Vec3::new(
                (ndc.0 * aspect * tan_half) as f32,
                (ndc.1 * tan_half) as f32,
                -1.0,
            );
            let direction = inverse.transform_vector3(view_direction);
            (
                self.eye_plane(),
                DVec3::new(
                    f64::from(direction.x),
                    f64::from(direction.y),
                    f64::from(direction.z),
                ),
            )
        }
    }

    fn ray_plane_point(&self, ndc: (f64, f64), aspect: f64) -> Option<DVec3> {
        let (origin, direction) = self.ray_ndc(ndc, aspect);
        if direction.z.abs() < 1e-9 {
            return None;
        }
        let t = -origin.z / direction.z;
        if t <= 0.0 {
            return None;
        }
        let hit = origin + direction * t;
        Some(DVec3::new(hit.x, hit.y, 0.0))
    }

    pub fn pick_plane(&self, pixel: (f64, f64), viewport: (u32, u32)) -> Option<DVec3> {
        let aspect = f64::from(viewport.0.max(1)) / f64::from(viewport.1.max(1));
        let ndc = (
            pixel.0 / f64::from(viewport.0.max(1)) * 2.0 - 1.0,
            1.0 - pixel.1 / f64::from(viewport.1.max(1)) * 2.0,
        );
        self.ray_plane_point(ndc, aspect)
    }

    /// Pan by a screen delta (drag) in plane meters. The ground follows the
    /// cursor, like Google Earth: dragging moves the map with the pointer.
    pub fn pan_pixels(&mut self, delta: (f64, f64), viewport: (u32, u32)) {
        let height = f64::from(viewport.1.max(1));
        let (horizontal, vertical) = if self.is_orthographic() {
            let per_pixel = self.distance / height;
            (per_pixel, per_pixel)
        } else {
            // Ground scale at the centre of a perspective view. Screen-y is
            // compressed by the tilt: the flatter the view, the more ground a
            // vertical pixel covers.
            let centre = DVec3::new(f64::from(self.center.x), f64::from(self.center.y), 0.0);
            let distance = (self.eye_plane() - centre).length().max(1.0);
            let per_pixel = distance * 2.0 * (std::f64::consts::FRAC_PI_4 / 2.0).tan() / height;
            (per_pixel, per_pixel / self.pitch.sin().max(0.2))
        };
        let (sin, cos) = self.heading.sin_cos();
        let dx = -delta.0 * horizontal;
        let dy = delta.1 * vertical;
        self.center.x += (dx * cos - dy * sin) as f32;
        self.center.y += (dx * sin + dy * cos) as f32;
        self.clamp_center();
    }

    /// Rotates the map. Positive `radians` turn the imagery to the right, the
    /// same sense as dragging the map. The orthographic 2D view is north-up by
    /// construction, so it ignores rotation.
    pub fn rotate_by(&mut self, radians: f64) {
        if self.is_orthographic() || !radians.is_finite() {
            return;
        }
        self.heading = wrap_angle(self.heading + radians);
    }

    /// Tilts the perspective view. Positive `radians` raise the eye towards a
    /// top-down view, negative tilt towards the horizon.
    pub fn tilt_by(&mut self, radians: f64) {
        if self.is_orthographic() || !radians.is_finite() {
            return;
        }
        self.pitch = (self.pitch + radians).clamp(10f64.to_radians(), 89.5f64.to_radians());
    }

    /// Looking around the eye is a 3D-globe gesture; the flat views ignore it.
    pub fn look_by(&mut self, _d_heading: f64, _d_pitch: f64) {}

    pub fn set_heading(&mut self, radians: f64) {
        if radians.is_finite() {
            self.heading = wrap_angle(radians);
        }
    }

    /// Google Earth's shift-drag: horizontal rotation plus tilt.
    pub fn orbit(&mut self, delta_x: f64, delta_y: f64) {
        self.rotate_by(delta_x * 0.005);
        self.tilt_by(-delta_y * 0.005);
    }

    /// Moves the view along the screen axes, in metres on the map. Positive
    /// `forward` moves towards the top of the screen, positive `right` to the
    /// right.
    pub fn pan_view(&mut self, forward_m: f64, right_m: f64) {
        if !forward_m.is_finite() || !right_m.is_finite() {
            return;
        }
        let (sin, cos) = self.heading.sin_cos();
        // Screen up and screen right in plane coordinates (see `pan_pixels`).
        let up = Vec2::new(-sin as f32, cos as f32);
        let right = Vec2::new(cos as f32, sin as f32);
        self.center += up * forward_m as f32 + right * right_m as f32;
        self.clamp_center();
    }

    pub fn zoom(&mut self, factor: f64) {
        if !factor.is_finite() || factor <= 0.0 {
            return;
        }
        self.distance = (self.distance * factor).clamp(MIN_SCALE, MAX_SCALE);
    }

    pub fn zoom_at(&mut self, pixel: (f64, f64), viewport: (u32, u32), factor: f64) {
        let Some(before) = self.pick_plane(pixel, viewport) else {
            self.zoom(factor);
            return;
        };
        self.zoom(factor);
        let Some(after) = self.pick_plane(pixel, viewport) else {
            return;
        };
        // Keep the point under the cursor anchored.
        self.center.x += (before.x - after.x) as f32;
        self.center.y += (before.y - after.y) as f32;
        self.clamp_center();
    }

    /// Fly to a geographic target, keeping the current zoom.
    pub fn fly_to(&mut self, point: &GeoPoint, distance: f64) {
        let plane = lon_lat_to_plane(point);
        self.center = Vec2::new(plane.x as f32, plane.y as f32);
        self.distance = distance.clamp(MIN_SCALE, MAX_SCALE);
        self.clamp_center();
    }

    /// Eye position in plane space, used for eye-relative tile translation.
    pub fn eye_plane(&self) -> DVec3 {
        let center = DVec3::new(f64::from(self.center.x), f64::from(self.center.y), 0.0);
        if self.is_orthographic() {
            center + DVec3::new(0.0, 0.0, ORTHO_EYE_HEIGHT)
        } else {
            let (sin_heading, cos_heading) = self.heading.sin_cos();
            let (sin_pitch, cos_pitch) = self.pitch.sin_cos();
            center
                + DVec3::new(sin_heading * cos_pitch, -cos_heading * cos_pitch, sin_pitch)
                    * self.distance
        }
    }

    fn clamp_center(&mut self) {
        let half = (WORLD_WIDTH_METERS / 2.0) as f32;
        self.center.x = self.center.x.clamp(-half, half);
        self.center.y = self.center.y.clamp(-half, half);
    }

    pub fn from_camera(camera: &Camera, mode: SceneMode) -> Self {
        Self::from_pose(&camera.pose()).with_mode(mode)
    }
}

fn bounds(points: &[DVec3]) -> PlaneRect {
    let mut min = Vec2::new(f32::INFINITY, f32::INFINITY);
    let mut max = Vec2::new(f32::NEG_INFINITY, f32::NEG_INFINITY);
    for point in points {
        min.x = min.x.min(point.x as f32);
        min.y = min.y.min(point.y as f32);
        max.x = max.x.max(point.x as f32);
        max.y = max.y.max(point.y as f32);
    }
    PlaneRect { min, max }
}

fn wrap_angle(radians: f64) -> f64 {
    let two_pi = std::f64::consts::TAU;
    let mut value = radians % two_pi;
    if value > std::f64::consts::PI {
        value -= two_pi;
    } else if value < -std::f64::consts::PI {
        value += two_pi;
    }
    value
}

/// One flat quad for a Mercator tile. The plane needs no subdivision: tiles at
/// different LOD levels join seamlessly because the surface is flat.
pub fn build_flat_tile_mesh(id: TileId) -> crate::tiles::TileMesh {
    let rect = PlaneRect::from_tile(id);
    let center = rect.center();
    let corners = [
        (Vec2::new(rect.min.x, rect.min.y), [0.0f32, 1.0]),
        (Vec2::new(rect.max.x, rect.min.y), [1.0, 1.0]),
        (Vec2::new(rect.max.x, rect.max.y), [1.0, 0.0]),
        (Vec2::new(rect.min.x, rect.max.y), [0.0, 0.0]),
    ];
    crate::tiles::TileMesh {
        positions: corners
            .iter()
            .map(|(point, _)| [point.x - center.x, point.y - center.y, 0.0])
            .collect(),
        uvs: corners.iter().map(|(_, uv)| *uv).collect(),
        indices: vec![0, 1, 2, 0, 2, 3],
        center_ecef: DVec3::new(f64::from(center.x), f64::from(center.y), 0.0),
    }
}

/// Tiles needed to cover the view at roughly one texel per pixel.
pub fn select_flat_tiles(
    camera: &FlatCamera,
    viewport: (u32, u32),
    max_level: u8,
    max_tiles: usize,
    texture_ready: &dyn Fn(TileId) -> bool,
) -> Vec<crate::tiles::SelectedTile> {
    let visible = camera.visible_rect(viewport);
    let aspect = f64::from(viewport.0.max(1)) / f64::from(viewport.1.max(1));
    let visible_width = if camera.is_orthographic() {
        camera.distance * aspect
    } else {
        f64::from(visible.max.x - visible.min.x)
    }
    .max(1.0);
    let meters_per_pixel = visible_width / f64::from(viewport.0.max(1));
    let target_tile_meters = 256.0 * meters_per_pixel;
    let mut level = (WORLD_WIDTH_METERS / target_tile_meters)
        .log2()
        .round()
        .max(0.0) as u8;
    level = level.min(max_level);

    // Very large views can exceed the budget at the chosen level; step out until
    // they fit. Truncating the tile list instead would leave the right and
    // bottom of the view uncovered.
    while level > 0 && tile_count_at_level(level, &visible) > max_tiles {
        level -= 1;
    }
    if tile_count_at_level(level, &visible) > max_tiles {
        return Vec::new();
    }
    let tiles = tiles_at_level(level, &visible);

    tiles
        .into_iter()
        .map(|id| crate::tiles::SelectedTile {
            id,
            texture: nearest_ready_texture(id, texture_ready),
        })
        .collect()
}

/// Inclusive/exclusive tile ranges covered by a plane rectangle.
fn tile_ranges(level: u8, visible: &PlaneRect) -> (u32, u32, u32, u32) {
    let count = 1u32 << level;
    let tile_width = (WORLD_WIDTH_METERS / f64::from(count)) as f32;
    let half_world = (WORLD_WIDTH_METERS / 2.0) as f32;
    let min_x = ((visible.min.x + half_world) / tile_width).floor().max(0.0) as u32;
    let max_x = ((visible.max.x + half_world) / tile_width)
        .ceil()
        .clamp(0.0, count as f32) as u32;
    let max_y_plane = (WORLD_HEIGHT_METERS / 2.0) as f32;
    let min_y = ((max_y_plane - visible.max.y) / tile_width)
        .floor()
        .max(0.0) as u32;
    let max_y = ((max_y_plane - visible.min.y) / tile_width)
        .ceil()
        .clamp(0.0, count as f32) as u32;
    (
        min_x.min(count),
        max_x.max(min_x).min(count),
        min_y.min(count),
        max_y.max(min_y).min(count),
    )
}

fn tile_count_at_level(level: u8, visible: &PlaneRect) -> usize {
    let (min_x, max_x, min_y, max_y) = tile_ranges(level, visible);
    usize::try_from((max_x - min_x) * (max_y - min_y)).unwrap_or(usize::MAX)
}

fn tiles_at_level(level: u8, visible: &PlaneRect) -> Vec<TileId> {
    let (min_x, max_x, min_y, max_y) = tile_ranges(level, visible);
    let mut tiles = Vec::new();
    for x in min_x..max_x {
        for y in min_y..max_y {
            if let Some(id) = TileId::new(level, x, y) {
                tiles.push(id);
            }
        }
    }
    tiles
}

fn nearest_ready_texture(
    id: TileId,
    texture_ready: &dyn Fn(TileId) -> bool,
) -> Option<(TileId, [f32; 4])> {
    let mut current = Some(id);
    while let Some(candidate) = current {
        if texture_ready(candidate) {
            return Some((candidate, crate::geo::tile_uv_rect(id, candidate)));
        }
        current = candidate.parent();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(lon: f64, lat: f64) -> GeoPoint {
        GeoPoint::new(lon, lat).expect("valid")
    }

    #[test]
    fn plane_projection_round_trips() {
        for (lon, lat) in [(0.0, 0.0), (13.405, 52.52), (-170.0, -60.0), (179.0, 80.0)] {
            let plane = lon_lat_to_plane(&point(lon, lat));
            let back = plane_to_lon_lat(plane);
            assert!((back.lon - lon).abs() < 1e-6, "lon {lon} -> {}", back.lon);
            assert!((back.lat - lat).abs() < 1e-6, "lat {lat} -> {}", back.lat);
        }
    }

    #[test]
    fn east_is_positive_x_and_north_positive_y() {
        let origin = lon_lat_to_plane(&point(0.0, 0.0));
        let east = lon_lat_to_plane(&point(10.0, 0.0));
        let north = lon_lat_to_plane(&point(0.0, 10.0));
        assert!(east.x > origin.x);
        assert!((east.y - origin.y).abs() < 1.0);
        assert!(north.y > origin.y);
        assert!((north.x - origin.x).abs() < 1.0);
    }

    #[test]
    fn tile_rects_tile_the_world() {
        let root = PlaneRect::from_tile(TileId::root());
        assert!((root.min.x + (WORLD_WIDTH_METERS / 2.0) as f32).abs() < 2.0);
        assert!((root.width() - WORLD_WIDTH_METERS as f32).abs() < 4.0);
        let children: Vec<PlaneRect> = TileId::root()
            .children()
            .into_iter()
            .flatten()
            .map(PlaneRect::from_tile)
            .collect();
        assert_eq!(children.len(), 4);
        assert!(children.iter().all(|child| root.intersects(child)));
    }

    #[test]
    fn orthographic_camera_projects_the_centre_and_picks_it_back() {
        let camera = FlatCamera {
            center: Vec2::new(0.0, 0.0),
            distance: 4_000_000.0,
            ..FlatCamera::default()
        };
        let viewport = (1000, 500);
        let screen = camera.project(DVec3::ZERO, viewport).expect("visible");
        assert!((screen.x - 500.0).abs() < 1.0);
        assert!((screen.y - 250.0).abs() < 1.0);
        let picked = camera
            .pick_plane((500.0, 250.0), viewport)
            .expect("plane hit");
        assert!(picked.x.abs() < 1.0 && picked.y.abs() < 1.0);
    }

    #[test]
    fn orthographic_zoom_changes_scale_and_pan_moves_the_centre() {
        let mut camera = FlatCamera {
            center: Vec2::new(0.0, 0.0),
            distance: 4_000_000.0,
            ..FlatCamera::default()
        };
        let viewport = (1000, 500);
        let east = DVec3::new(1_000_000.0, 0.0, 0.0);
        let before = camera.project(east, viewport).expect("visible");
        camera.zoom(0.5);
        let after = camera.project(east, viewport).expect("visible");
        assert!(after.x > before.x, "zooming in moves east further right");

        camera.pan_pixels((100.0, 0.0), viewport);
        assert!(camera.center.x < 0.0, "dragging right pans west");
    }

    #[test]
    fn columbus_camera_picks_the_plane_below_the_cursor() {
        let camera = FlatCamera {
            center: Vec2::new(0.0, 0.0),
            distance: 2_000_000.0,
            pitch: 45f64.to_radians(),
            ..FlatCamera::default()
        };
        let viewport = (1000, 600);
        let picked = camera.pick_plane((500.0, 300.0), viewport).expect("hit");
        assert!(picked.x.abs() < 100.0 && picked.y.abs() < 100.0);
    }

    #[test]
    fn flat_selection_matches_the_zoom_level() {
        let mut camera = FlatCamera {
            center: Vec2::new(0.0, 0.0),
            distance: 40_000_000.0,
            ..FlatCamera::default()
        };
        let far = select_flat_tiles(&camera, (1280, 800), 19, 120, &|_| false);
        camera.distance = 20_000.0;
        let near = select_flat_tiles(&camera, (1280, 800), 19, 120, &|_| false);
        assert!(!far.is_empty() && !near.is_empty());
        assert!(
            near.iter().map(|tile| tile.id.z).max().unwrap_or(0)
                > far.iter().map(|tile| tile.id.z).max().unwrap_or(0)
        );
        assert!(near.len() <= 120 && far.len() <= 120);
    }

    /// The flat selector used to truncate its tile list when the budget was too
    /// small, which left the right and bottom of the view with no imagery at
    /// all. It has to step out to a coarser level instead.
    #[test]
    fn flat_selection_covers_the_view_within_the_budget() {
        let viewport = (1280, 800);
        for distance in [40_000_000.0, 2_000_000.0, 20_000.0] {
            for max_tiles in [1, 2, 4, 16, 60] {
                let camera = FlatCamera {
                    center: Vec2::new(-1_200_000.0, 340_000.0),
                    distance,
                    ..FlatCamera::default()
                };
                let tiles = select_flat_tiles(&camera, viewport, 19, max_tiles, &|_| false);
                assert!(
                    tiles.len() <= max_tiles,
                    "{distance} m / {max_tiles} tiles: selected {}",
                    tiles.len()
                );
                let visible = camera.visible_rect(viewport);
                // Zoomed far out the view extends past the edges of the world;
                // there is no imagery there, only the globe view's space colour.
                let half = (WORLD_WIDTH_METERS / 2.0) as f32;
                let world = PlaneRect {
                    min: Vec2::new(-half, -half),
                    max: Vec2::new(half, half),
                };
                let mut samples = 0;
                let mut uncovered = Vec::new();
                for ix in 0..=8 {
                    for iy in 0..=8 {
                        let t = Vec2::new(ix as f32 / 8.0, iy as f32 / 8.0);
                        let point = visible.min + (visible.max - visible.min) * t;
                        if !world.contains(point) {
                            continue;
                        }
                        samples += 1;
                        if !tiles
                            .iter()
                            .any(|tile| PlaneRect::from_tile(tile.id).contains(point))
                        {
                            uncovered.push(point);
                        }
                    }
                }
                assert!(
                    samples > 0,
                    "{distance} m / {max_tiles} tiles: nothing to check"
                );
                assert!(
                    uncovered.is_empty(),
                    "{distance} m / {max_tiles} tiles: {} of {samples} view samples uncovered",
                    uncovered.len()
                );
            }
        }
    }

    #[test]
    fn flat_tile_mesh_is_a_quad_centred_on_the_tile() {
        let mesh = build_flat_tile_mesh(TileId::new(3, 4, 2).expect("tile"));
        assert_eq!(mesh.positions.len(), 4);
        assert_eq!(mesh.indices.len(), 6);
        assert!(mesh.positions.iter().all(|position| position[2] == 0.0));
    }
}
