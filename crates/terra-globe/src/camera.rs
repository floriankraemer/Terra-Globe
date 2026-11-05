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

    pub fn orbit(&mut self, delta_x: f64, delta_y: f64) {
        self.heading = wrap_angle(self.heading - delta_x * 0.005);
        self.pitch = (self.pitch - delta_y * 0.005).clamp(MIN_PITCH, MAX_PITCH);
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
