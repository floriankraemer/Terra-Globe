//! Globe camera: orbit around an ellipsoid point with reverse-Z, infinite-far
//! projection, and relative-to-eye (RTC) transforms.

use glam::{DVec3, Mat4, Vec3, Vec4};

use crate::geo::{Geodetic, ecef_to_geodetic, enu_axes, geodetic_to_ecef};

/// Where a projected point landed on screen.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenPoint {
    pub x: f64,
    pub y: f64,
    /// NDC depth in `0..=1`; larger is closer (reverse-Z).
    pub depth: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraPose {
    pub lon_deg: f64,
    pub lat_deg: f64,
    pub distance: f64,
    pub heading: f64,
    pub pitch: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraAnimation {
    from: CameraPose,
    to: CameraPose,
    start_seconds: f64,
    duration_seconds: f64,
}

const MIN_DISTANCE: f64 = 8.0;
const MAX_DISTANCE: f64 = 40_000_000.0;
const MIN_PITCH: f64 = 10f64.to_radians();
const MAX_PITCH: f64 = 89.5f64.to_radians();

#[derive(Debug, Clone)]
pub struct Camera {
    pub target_lon_deg: f64,
    pub target_lat_deg: f64,
    /// Distance from the target point to the eye, in meters.
    pub distance: f64,
    /// View direction around the target: 0 = looking north, positive clockwise.
    pub heading: f64,
    /// Elevation of the eye above the target's tangent plane.
    pub pitch: f64,
    /// Vertical field of view.
    pub fov_y: f64,
    pub near: f64,
    pub(crate) animation: Option<CameraAnimation>,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            target_lon_deg: 10.0,
            target_lat_deg: 35.0,
            distance: 20_000_000.0,
            heading: 0.0,
            pitch: 45f64.to_radians(),
            fov_y: 45f64.to_radians(),
            near: 1.0,
            animation: None,
        }
    }
}

impl Camera {
    pub fn target_geodetic(&self) -> Geodetic {
        Geodetic::from_degrees(self.target_lon_deg, self.target_lat_deg, 0.0)
    }

    pub fn target_ecef(&self) -> DVec3 {
        geodetic_to_ecef(self.target_geodetic())
    }

    pub fn pose(&self) -> CameraPose {
        CameraPose {
            lon_deg: self.target_lon_deg,
            lat_deg: self.target_lat_deg,
            distance: self.distance,
            heading: self.heading,
            pitch: self.pitch,
        }
    }

    pub fn apply_pose(&mut self, pose: CameraPose) {
        // Normalize after a fly-to, which interpolates through unwrapped
        // longitudes (e.g. 170 -> 190) to take the short way round.
        self.target_lon_deg = wrap_angle(pose.lon_deg.to_radians()).to_degrees();
        self.target_lat_deg = pose.lat_deg;
        self.distance = pose.distance;
        self.heading = pose.heading;
        self.pitch = pose.pitch;
    }

    /// Camera position in ECEF, eye-relative rendering's origin.
    pub fn eye_ecef(&self) -> DVec3 {
        let target = self.target_ecef();
        let (east, north, up) = enu_axes(target);
        let (sin_heading, cos_heading) = self.heading.sin_cos();
        let (sin_pitch, cos_pitch) = self.pitch.sin_cos();
        // The eye sits opposite the view direction, so heading 0 (looking north)
        // places the camera south of the target and puts east on the right.
        let direction =
            east * (sin_heading * cos_pitch) - north * (cos_heading * cos_pitch) + up * sin_pitch;
        target + direction * self.distance
    }

    pub fn eye_height(&self) -> f64 {
        ecef_to_geodetic(self.eye_ecef()).height
    }

    /// Rotation-only view matrix: the eye is the origin, so no translation is
    /// applied and no large coordinates ever reach the GPU.
    pub fn view_rotation(&self) -> Mat4 {
        let eye = self.eye_ecef();
        let target = self.target_ecef();
        let forward = (target - eye).normalize();

        let (_, _, up_d) = enu_axes(eye);
        let mut up = up_d;
        // Near the poles the local up and the view direction can be parallel.
        if forward.dot(up).abs() > 0.999 {
            let (east, _, _) = enu_axes(eye);
            up = east.cross(forward).normalize();
        }

        glam::camera::rh::view::look_to_mat4(Vec3::ZERO, forward.as_vec3(), up.as_vec3())
    }

    /// Reverse-Z, infinite-far projection: near maps to 1.0, infinity to 0.0.
    pub fn projection(&self, aspect: f64) -> Mat4 {
        let f = 1.0 / (self.fov_y / 2.0).tan();
        let aspect = if aspect.is_finite() && aspect > 0.0 {
            aspect
        } else {
            1.0
        };
        let n = self.near;
        // Columns: (f/aspect,0,0,0) (0,f,0,0) (0,0,0,-1) (0,0,n,0)
        Mat4::from_cols(
            Vec4::new((f / aspect) as f32, 0.0, 0.0, 0.0),
            Vec4::new(0.0, f as f32, 0.0, 0.0),
            Vec4::new(0.0, 0.0, 0.0, -1.0),
            Vec4::new(0.0, 0.0, n as f32, 0.0),
        )
    }

    pub fn view_projection(&self, aspect: f64) -> Mat4 {
        self.projection(aspect) * self.view_rotation()
    }

    /// Projects an ECEF point into viewport pixels.
    pub fn project(&self, point_ecef: DVec3, viewport: (u32, u32)) -> Option<ScreenPoint> {
        let (width, height) = viewport;
        if width == 0 || height == 0 {
            return None;
        }
        let aspect = f64::from(width) / f64::from(height);
        let relative = (point_ecef - self.eye_ecef()).as_vec3();
        let clip = self.view_projection(aspect) * relative.extend(1.0);
        if clip.w <= 0.0 {
            return None;
        }
        let ndc = clip.truncate() / clip.w;
        if !(-1.0..=1.0).contains(&ndc.x) || !(-1.0..=1.0).contains(&ndc.y) {
            return None;
        }
        Some(ScreenPoint {
            x: (f64::from(ndc.x) * 0.5 + 0.5) * f64::from(width),
            y: (1.0 - (f64::from(ndc.y) * 0.5 + 0.5)) * f64::from(height),
            depth: f64::from(ndc.z),
        })
    }

    /// Ray through a viewport pixel, in ECEF.
    pub fn ray_at(&self, pixel: (f64, f64), viewport: (u32, u32)) -> (DVec3, DVec3) {
        let width = f64::from(viewport.0.max(1));
        let height = f64::from(viewport.1.max(1));
        let ndc_x = (pixel.0 / width) * 2.0 - 1.0;
        let ndc_y = 1.0 - (pixel.1 / height) * 2.0;
        let tan_half = (self.fov_y / 2.0).tan();
        let aspect = width / height;

        // View space ray, then rotate into world space with the inverse view
        // rotation (transpose of a pure rotation).
        let view_ray = Vec3::new(
            (ndc_x * aspect * tan_half) as f32,
            (ndc_y * tan_half) as f32,
            -1.0,
        );
        let rotation = self.view_rotation();
        let world_ray = rotation.inverse().transform_vector3(view_ray);
        let eye = self.eye_ecef();
        (
            eye,
            DVec3::new(
                f64::from(world_ray.x),
                f64::from(world_ray.y),
                f64::from(world_ray.z),
            ),
        )
    }

    /// Unit vector from the eye towards the target.
    pub fn view_forward(&self) -> DVec3 {
        (self.target_ecef() - self.eye_ecef()).normalize()
    }

    /// The point on the ellipsoid under a viewport pixel, if the ray hits it.
    pub fn pick_surface(&self, pixel: (f64, f64), viewport: (u32, u32)) -> Option<DVec3> {
        let (origin, direction) = self.ray_at(pixel, viewport);
        crate::geo::ray_ellipsoid_point(origin, direction)
    }

    /// The two ground axes the screen maps to, as `(forward, right)` unit
    /// vectors in the local east/north frame at the target.
    ///
    /// `forward` is the direction the camera looks (screen up), `right` is
    /// screen right. Both stay well defined at any tilt because the horizontal
    /// part of the view direction only depends on the heading.
    fn ground_axes(&self) -> ((f64, f64), (f64, f64)) {
        let (sin_h, cos_h) = self.heading.sin_cos();
        ((-sin_h, cos_h), (cos_h, sin_h))
    }

    /// Rotates the view around the target. Positive `radians` turn the imagery
    /// to the right, which is what dragging the globe to the right does.
    pub fn rotate_by(&mut self, radians: f64) {
        if radians.is_finite() {
            self.heading = wrap_angle(self.heading + radians);
        }
    }

    /// Tilts the view. Positive `radians` raise the eye towards a top-down
    /// view, negative tilts towards the horizon.
    pub fn tilt_by(&mut self, radians: f64) {
        if radians.is_finite() {
            self.pitch = (self.pitch + radians).clamp(MIN_PITCH, MAX_PITCH);
            self.ensure_above_surface();
        }
    }

    /// Google Earth's shift-drag: horizontal rotation plus tilt.
    pub fn orbit(&mut self, delta_x: f64, delta_y: f64) {
        self.rotate_by(delta_x * 0.005);
        // Dragging down brings the horizon down into view, i.e. tilts up.
        self.tilt_by(-delta_y * 0.005);
    }

    pub fn set_heading(&mut self, radians: f64) {
        if radians.is_finite() {
            self.heading = wrap_angle(radians);
        }
    }

    pub fn set_pitch(&mut self, radians: f64) {
        if radians.is_finite() {
            self.pitch = radians.clamp(MIN_PITCH, MAX_PITCH);
            self.ensure_above_surface();
        }
    }

    /// Stops a running fly-to.
    pub fn stop(&mut self) {
        self.animation = None;
    }

    /// Moves the view across the ground without changing its orientation or
    /// altitude: the eye and the target translate together.
    pub fn pan_ground(&mut self, east_m: f64, north_m: f64) {
        if !east_m.is_finite() || !north_m.is_finite() {
            return;
        }
        let lat = self.target_lat_deg.to_radians();
        // Near the poles a metre of easting is many degrees of longitude; the
        // floor keeps a pan from flinging the target around the pole.
        let cos_lat = lat.cos().max(0.02);
        let dlat = (north_m / crate::geo::WGS84_A).to_degrees();
        let dlon = (east_m / (crate::geo::WGS84_A * cos_lat)).to_degrees();
        self.target_lat_deg = (self.target_lat_deg + dlat).clamp(-89.5, 89.5);
        self.target_lon_deg = wrap_angle((self.target_lon_deg + dlon).to_radians()).to_degrees();
        self.ensure_above_surface();
    }

    /// Moves the view along the screen axes, in metres on the ground. Positive
    /// `forward` moves towards the part of the globe the camera faces, positive
    /// `right` moves to the right of the screen.
    pub fn pan_view(&mut self, forward_m: f64, right_m: f64) {
        let ((forward_east, forward_north), (right_east, right_north)) = self.ground_axes();
        self.pan_ground(
            forward_east * forward_m + right_east * right_m,
            forward_north * forward_m + right_north * right_m,
        );
    }

    /// Metres of ground per screen pixel at a pixel, as
    /// `(horizontal, vertical)`.
    ///
    /// Screen-x is not foreshortened by tilt (it moves along a circle around the
    /// eye), screen-y is: the more obliquely the surface is viewed, the more
    /// ground one pixel of vertical movement covers.
    pub fn surface_scale_at(&self, pixel: (f64, f64), viewport: (u32, u32)) -> (f64, f64) {
        let screen_height = f64::from(viewport.1.max(1));
        let eye = self.eye_ecef();
        let (origin, direction) = self.ray_at(pixel, viewport);
        let point = crate::geo::ray_ellipsoid_point(origin, direction)
            .unwrap_or_else(|| self.target_ecef());
        let distance = (point - eye).length().max(1.0);
        let per_pixel = distance * 2.0 * (self.fov_y / 2.0).tan() / screen_height;
        let incidence = direction
            .normalize()
            .dot(crate::geo::ellipsoid_normal(point))
            .abs()
            .max(0.15);
        (per_pixel, per_pixel / incidence)
    }

    /// Drags the globe: the ground under `pixel` follows the cursor.
    pub fn pan_pixels(&mut self, pixel: (f64, f64), viewport: (u32, u32), delta: (f64, f64)) {
        if !delta.0.is_finite() || !delta.1.is_finite() {
            return;
        }
        let (per_pixel_x, per_pixel_y) = self.surface_scale_at(pixel, viewport);
        // Dragging down moves the view forward (up the screen), dragging right
        // moves it to the left: the imagery stays under the cursor.
        self.pan_view(delta.1 * per_pixel_y, -delta.0 * per_pixel_x);
    }

    /// Zooms towards the surface point under `pixel`, Google Earth style.
    ///
    /// The eye dollies along the ray through the cursor, so that point stays
    /// where it is on screen and the view does not rotate at all. Moving the
    /// target towards the anchor by `1 - factor` is exactly that dolly: with
    /// `d' = d * factor`, the new eye is `eye + (1 - factor) * (anchor - eye)`.
    pub fn dolly_at(&mut self, pixel: (f64, f64), viewport: (u32, u32), factor: f64) {
        if !factor.is_finite() || factor <= 0.0 {
            return;
        }
        let factor = factor.clamp(0.1, 10.0);
        let anchor = self
            .pick_surface(pixel, viewport)
            .map(crate::geo::ecef_to_geodetic)
            .filter(|anchor| anchor.height.abs() < 1.0e6);
        if let Some(anchor) = anchor {
            let dlon =
                wrap_angle((anchor.lon_deg() - self.target_lon_deg).to_radians()).to_degrees();
            let weight = 1.0 - factor;
            self.target_lon_deg =
                wrap_angle((self.target_lon_deg + dlon * weight).to_radians()).to_degrees();
            self.target_lat_deg = (self.target_lat_deg
                + (anchor.lat_deg() - self.target_lat_deg) * weight)
                .clamp(-89.5, 89.5);
        }
        self.distance = (self.distance * factor).clamp(MIN_DISTANCE, MAX_DISTANCE);
        self.ensure_above_surface();
    }

    /// Multiplies the distance; `factor < 1` zooms in.
    pub fn zoom(&mut self, factor: f64) {
        if !factor.is_finite() || factor <= 0.0 {
            return;
        }
        self.distance = (self.distance * factor).clamp(MIN_DISTANCE, MAX_DISTANCE);
        self.ensure_above_surface();
    }

    /// Keeps the eye above the ellipsoid after pitch/zoom changes.
    pub fn ensure_above_surface(&mut self) {
        for _ in 0..8 {
            let height = self.eye_height();
            if height >= 1.0 {
                return;
            }
            self.distance = (self.distance * 1.05).clamp(MIN_DISTANCE, MAX_DISTANCE);
            if (self.distance - MAX_DISTANCE).abs() < f64::EPSILON {
                break;
            }
        }
    }

    pub fn fly_to(&mut self, pose: CameraPose, duration_seconds: f64, now_seconds: f64) {
        if duration_seconds <= 0.0 {
            self.apply_pose(pose);
            self.animation = None;
            return;
        }
        let mut to = pose;
        // Shortest path across the antimeridian.
        let delta_lon = wrap_angle((to.lon_deg - self.target_lon_deg).to_radians()).to_degrees();
        to.lon_deg = self.target_lon_deg + delta_lon;
        self.animation = Some(CameraAnimation {
            from: self.pose(),
            to,
            start_seconds: now_seconds,
            duration_seconds,
        });
    }

    pub fn is_animating(&self) -> bool {
        self.animation.is_some()
    }

    /// Advances an active fly-to. Returns true while the camera is moving.
    pub fn update(&mut self, now_seconds: f64) -> bool {
        let Some(animation) = self.animation else {
            return false;
        };
        let elapsed = (now_seconds - animation.start_seconds) / animation.duration_seconds;
        let t = elapsed.clamp(0.0, 1.0);
        // Smoothstep keeps the start and end gentle, like Cesium's default ease.
        let eased = t * t * (3.0 - 2.0 * t);
        self.target_lon_deg = lerp(animation.from.lon_deg, animation.to.lon_deg, eased);
        self.target_lat_deg = lerp(animation.from.lat_deg, animation.to.lat_deg, eased);
        self.distance = lerp_log(animation.from.distance, animation.to.distance, eased);
        self.heading = lerp(animation.from.heading, animation.to.heading, eased);
        self.pitch = lerp(animation.from.pitch, animation.to.pitch, eased);
        if t >= 1.0 {
            self.animation = None;
            self.apply_pose(animation.to);
            self.ensure_above_surface();
            return false;
        }
        true
    }
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// Geometric interpolation for distances reads as constant speed on screen.
fn lerp_log(a: f64, b: f64, t: f64) -> f64 {
    if a <= 0.0 || b <= 0.0 {
        return lerp(a, b, t);
    }
    (a.ln() + (b.ln() - a.ln()) * t).exp()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geo::{MERCATOR_MAX_LAT_DEG, geodetic_to_ecef, is_point_visible};

    #[test]
    fn near_plane_maps_to_one_and_far_to_zero() {
        let camera = Camera {
            near: 10.0,
            ..Camera::default()
        };
        let projection = camera.projection(1.0);
        // View space: forward is -Z.
        let near_clip = projection * Vec4::new(0.0, 0.0, -10.0, 1.0);
        assert!((near_clip.z / near_clip.w - 1.0).abs() < 1e-6);
        let far_clip = projection * Vec4::new(0.0, 0.0, -1.0e9, 1.0);
        assert!(far_clip.z / far_clip.w < 1e-4);
        assert!(far_clip.z / far_clip.w > 0.0);
    }

    #[test]
    fn heading_zero_looks_north_with_east_on_the_right() {
        let camera = Camera {
            distance: 1_000_000.0,
            pitch: 60f64.to_radians(),
            ..Camera::default()
        };
        let target = camera.target_geodetic();
        let center = camera
            .project(crate::geo::geodetic_to_ecef(target), (1280, 800))
            .expect("target visible");
        let east_point = crate::geo::Geodetic {
            lon: target.lon + 0.01,
            lat: target.lat,
            height: 0.0,
        };
        let east = camera
            .project(crate::geo::geodetic_to_ecef(east_point), (1280, 800))
            .expect("east point visible");
        assert!(
            east.x > center.x,
            "east must be to the right: {east:?} vs {center:?}"
        );
    }

    #[test]
    fn target_projects_to_the_viewport_center() {
        let camera = Camera::default();
        let point = geodetic_to_ecef(camera.target_geodetic());
        let screen = camera
            .project(point, (1280, 800))
            .expect("target must be on screen");
        assert!((screen.x - 640.0).abs() < 1.0, "x was {}", screen.x);
        assert!((screen.y - 400.0).abs() < 1.0, "y was {}", screen.y);
    }

    #[test]
    fn ray_through_center_points_at_the_target() {
        let camera = Camera::default();
        let (origin, direction) = camera.ray_at((640.0, 400.0), (1280, 800));
        let target = camera.target_ecef();
        let to_target = (target - origin).normalize();
        assert!(direction.normalize().dot(to_target) > 0.9999);
    }

    #[test]
    fn orbit_wraps_heading_and_clamps_pitch() {
        let mut camera = Camera::default();
        camera.orbit(0.0, 1.0e9);
        assert!(camera.pitch >= MIN_PITCH);
        camera.heading = 3.0;
        camera.orbit(-2000.0, 0.0);
        assert!(camera.heading.abs() <= std::f64::consts::PI);
    }

    fn screen_x_of(camera: &Camera, point: crate::geo::Geodetic) -> f64 {
        camera
            .project(geodetic_to_ecef(point), (1280, 800))
            .expect("point on screen")
            .x
    }

    /// Google Earth's shift-drag grabs the scene: whatever is under the cursor
    /// moves with it, so rotating to the right moves the imagery right.
    #[test]
    fn rotating_right_moves_the_imagery_right() {
        let mut camera = Camera {
            distance: 6_000_000.0,
            pitch: 60f64.to_radians(),
            ..Camera::default()
        };
        let north = Geodetic::from_degrees(camera.target_lon_deg, camera.target_lat_deg + 2.0, 0.0);
        let before = screen_x_of(&camera, north);
        camera.orbit(100.0, 0.0);
        let after = screen_x_of(&camera, north);
        assert!(
            after > before,
            "dragging right must move the scene right: {before} -> {after}"
        );
    }

    #[test]
    fn dragging_down_tilts_towards_the_horizon() {
        let mut camera = Camera {
            pitch: 60f64.to_radians(),
            ..Camera::default()
        };
        camera.orbit(0.0, 100.0);
        assert!(camera.pitch < 60f64.to_radians());
    }

    #[test]
    fn arrow_keys_move_along_the_current_heading() {
        let mut north_up = Camera {
            heading: 0.0,
            ..Camera::default()
        };
        let start = north_up.target_lat_deg;
        north_up.pan_view(100_000.0, 0.0);
        assert!(
            north_up.target_lat_deg > start,
            "forward must go north at heading 0"
        );
        assert!((north_up.target_lon_deg - 10.0).abs() < 1e-6);

        // Facing west (heading 90 degrees in this camera's convention), forward
        // has to go west, and right has to go north.
        let mut west = Camera {
            heading: std::f64::consts::FRAC_PI_2,
            ..Camera::default()
        };
        let lat = west.target_lat_deg;
        let lon = west.target_lon_deg;
        west.pan_view(100_000.0, 0.0);
        assert!(west.target_lon_deg < lon, "forward must go west");
        assert!((west.target_lat_deg - lat).abs() < 1e-6);
        west.pan_view(0.0, 100_000.0);
        assert!(west.target_lat_deg > lat, "right must go north");
    }

    #[test]
    fn arrow_keys_move_roughly_the_requested_distance() {
        let mut camera = Camera {
            target_lon_deg: 0.0,
            target_lat_deg: 0.0,
            ..Camera::default()
        };
        camera.pan_view(111_000.0, 0.0);
        assert!(
            (camera.target_lat_deg - 1.0).abs() < 0.01,
            "111 km of northing is about one degree, got {}",
            camera.target_lat_deg
        );
    }

    #[test]
    fn dragging_moves_the_ground_with_the_cursor() {
        let mut camera = Camera {
            distance: 2_000_000.0,
            pitch: 45f64.to_radians(),
            ..Camera::default()
        };
        let viewport = (1280, 800);
        let grab = (900.0, 300.0);
        let grabbed = camera
            .pick_surface(grab, viewport)
            .expect("the grab point is on the globe");
        let drag = (120.0, 0.0);
        camera.pan_pixels(grab, viewport, drag);
        let moved = camera
            .project(grabbed, viewport)
            .expect("the grabbed point is still on screen");
        // A grabbed point follows the cursor. North/south drags couple into the
        // picture's x on a sphere, so only the horizontal grab is asserted to
        // the pixel.
        assert!(
            (moved.x - (grab.0 + drag.0)).abs() < 15.0,
            "grabbed point should follow the cursor from {grab:?} by {drag:?}, landed at {moved:?}"
        );
        // And the view moved the other way: the point now under the original
        // pixel is the one that used to be west of the cursor.
        let (east, _, _) = crate::geo::enu_axes(grabbed);
        let under = camera
            .pick_surface(grab, viewport)
            .expect("still on the globe");
        assert!((grabbed - under).dot(east) > 0.0);
    }

    /// The reported bug: the wheel must zoom, and zooming must not rotate the
    /// view or shift the target when the cursor is in the middle.
    #[test]
    fn zooming_at_the_centre_changes_nothing_but_the_distance() {
        let mut camera = Camera {
            distance: 5_000_000.0,
            pitch: 45f64.to_radians(),
            heading: 0.7,
            ..Camera::default()
        };
        let before = camera.pose();
        camera.dolly_at((640.0, 400.0), (1280, 800), 0.9);
        assert!((camera.distance - before.distance * 0.9).abs() < 1.0);
        assert!((camera.pose().lon_deg - before.lon_deg).abs() < 1e-6);
        assert!((camera.pose().lat_deg - before.lat_deg).abs() < 1e-6);
        assert!((camera.heading - before.heading).abs() < 1e-9);
        assert!((camera.pitch - before.pitch).abs() < 1e-9);
    }

    #[test]
    fn zooming_keeps_the_point_under_the_cursor_in_place() {
        let mut camera = Camera {
            distance: 4_000_000.0,
            pitch: 50f64.to_radians(),
            ..Camera::default()
        };
        let viewport = (1280, 800);
        let cursor = (980.0, 260.0);
        let before = camera
            .project(
                camera.pick_surface(cursor, viewport).expect("on the globe"),
                viewport,
            )
            .expect("visible");

        for _ in 0..6 {
            camera.dolly_at(cursor, viewport, 0.85);
        }
        let anchor = camera.pick_surface(cursor, viewport).expect("on the globe");
        let after = camera.project(anchor, viewport).expect("visible");
        assert!(
            (after.x - before.x).abs() < 30.0 && (after.y - before.y).abs() < 30.0,
            "the anchor drifted from {before:?} to {after:?}"
        );
        // Zooming in towards a corner must not rotate the view.
        assert!((camera.pitch - 50f64.to_radians()).abs() < 1e-9);
    }

    #[test]
    fn zooming_never_looks_wrong_at_the_antimeridian() {
        let mut camera = Camera {
            target_lon_deg: 179.5,
            ..Camera::default()
        };
        camera.dolly_at((1100.0, 400.0), (1280, 800), 0.8);
        assert!(
            camera.target_lon_deg.abs() <= 180.0,
            "{}",
            camera.target_lon_deg
        );
    }

    #[test]
    fn zoom_is_clamped_and_keeps_the_eye_above_the_surface() {
        let mut camera = Camera {
            pitch: MIN_PITCH,
            distance: 20_000.0,
            ..Camera::default()
        };
        camera.zoom(1e-9);
        assert!(camera.distance >= MIN_DISTANCE);
        assert!(camera.eye_height() > 0.5);

        camera.zoom(1e12);
        assert!(camera.distance <= MAX_DISTANCE);
    }

    #[test]
    fn fly_to_moves_along_the_shortest_antimeridian_path() {
        let mut camera = Camera {
            target_lon_deg: 170.0,
            ..Camera::default()
        };
        let destination = CameraPose {
            lon_deg: -170.0,
            lat_deg: 0.0,
            distance: 1_000_000.0,
            heading: 0.0,
            pitch: 45f64.to_radians(),
        };
        camera.fly_to(destination, 1.0, 0.0);
        assert!(camera.update(0.5));
        // Halfway is past the antimeridian, not on the long way round via 0.
        assert!(
            camera.target_lon_deg.abs() > 179.0,
            "was {}",
            camera.target_lon_deg
        );
        assert!(!camera.update(1.0));
        assert!((camera.target_lon_deg - (-170.0)).abs() < 1e-9);
    }

    #[test]
    fn camera_can_see_its_target_at_every_zoom_level() {
        for distance in [10.0, 1_000.0, 1_000_000.0, 20_000_000.0] {
            let camera = Camera {
                distance,
                pitch: 45f64.to_radians(),
                ..Camera::default()
            };
            let eye = camera.eye_ecef();
            let target = camera.target_ecef();
            assert!(
                is_point_visible(eye, target),
                "target invisible at distance {distance}"
            );
        }
    }

    #[test]
    fn poles_are_projectable_when_in_view() {
        let camera = Camera {
            target_lon_deg: 0.0,
            target_lat_deg: 80.0,
            distance: 4_000_000.0,
            pitch: 60f64.to_radians(),
            ..Camera::default()
        };
        let pole = geodetic_to_ecef(Geodetic::from_degrees(0.0, 90.0, 0.0));
        assert!(
            camera.project(pole, (1280, 800)).is_some(),
            "north pole should be visible above lat {}",
            MERCATOR_MAX_LAT_DEG
        );
    }
}
