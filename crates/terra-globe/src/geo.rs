//! WGS84 geodesy: geodetic <-> ECEF, ray intersection, horizon tests, and
//! Web-Mercator tile math.
//!
//! Everything here is `f64` and free of GPU types, so it is unit-testable and
//! shared by the camera, tile selection, picking and the overlay projector.

use glam::DVec3;

/// Semi-major axis, meters.
pub const WGS84_A: f64 = 6_378_137.0;
/// Flattening.
pub const WGS84_F: f64 = 1.0 / 298.257_223_563;
/// Semi-minor axis, meters.
pub const WGS84_B: f64 = WGS84_A * (1.0 - WGS84_F);
/// First eccentricity squared.
pub const WGS84_E2: f64 = WGS84_F * (2.0 - WGS84_F);

/// Latitude limit of the Web-Mercator projection, in degrees.
pub const MERCATOR_MAX_LAT_DEG: f64 = 85.051_128_779_806_6;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Geodetic {
    /// Longitude in radians, [-pi, pi].
    pub lon: f64,
    /// Latitude in radians, [-pi/2, pi/2].
    pub lat: f64,
    /// Height above the ellipsoid in meters.
    pub height: f64,
}

impl Geodetic {
    pub fn from_degrees(lon_deg: f64, lat_deg: f64, height: f64) -> Self {
        Self {
            lon: lon_deg.to_radians(),
            lat: lat_deg.to_radians(),
            height,
        }
    }

    pub fn lon_deg(self) -> f64 {
        self.lon.to_degrees()
    }

    pub fn lat_deg(self) -> f64 {
        self.lat.to_degrees()
    }
}

/// Terrain height lookup `(lon_deg, lat_deg) -> meters`, `None` when no
/// terrain model is loaded (or it has no data there).
pub type GroundFn<'a> = dyn Fn(f64, f64) -> Option<f64> + 'a;

/// The "no terrain loaded" lookup: everything is clamped to the ellipsoid.
pub fn no_ground(_lon_deg: f64, _lat_deg: f64) -> Option<f64> {
    None
}

/// Height above the ellipsoid at which a point is drawn. Without terrain data
/// everything is clamped to the surface so nothing floats; with it, the KML
/// altitude mode decides (missing mode = clampToGround).
pub fn render_height(point: &terra_core::GeoPoint, ground: Option<f64>) -> f64 {
    use terra_core::AltitudeMode::{Absolute, ClampToGround, RelativeToGround};
    let Some(ground) = ground else { return 0.0 };
    match point.altitude_mode.unwrap_or(ClampToGround) {
        ClampToGround => ground,
        RelativeToGround => ground + point.altitude.unwrap_or(0.0),
        Absolute => point.altitude.unwrap_or(ground),
    }
}

/// Convenience conversion for domain points, at their render height.
pub fn geo_point_to_ecef(point: &terra_core::GeoPoint, ground: &GroundFn<'_>) -> DVec3 {
    geodetic_to_ecef(Geodetic::from_degrees(
        point.lon,
        point.lat,
        render_height(point, ground(point.lon, point.lat)),
    ))
}

/// Geodetic coordinates to Earth-centered, Earth-fixed meters.
pub fn geodetic_to_ecef(point: Geodetic) -> DVec3 {
    let (sin_lat, cos_lat) = point.lat.sin_cos();
    let (sin_lon, cos_lon) = point.lon.sin_cos();
    let prime_vertical = WGS84_A / (1.0 - WGS84_E2 * sin_lat * sin_lat).sqrt();
    DVec3::new(
        (prime_vertical + point.height) * cos_lat * cos_lon,
        (prime_vertical + point.height) * cos_lat * sin_lon,
        (prime_vertical * (1.0 - WGS84_E2) + point.height) * sin_lat,
    )
}

/// ECEF meters to geodetic coordinates (iterative, converges in a few steps for
/// every point this app can produce).
pub fn ecef_to_geodetic(position: DVec3) -> Geodetic {
    let lon = position.y.atan2(position.x);
    let horizontal = position.x.hypot(position.y);
    if horizontal < 1e-9 {
        let lat = if position.z >= 0.0 {
            std::f64::consts::FRAC_PI_2
        } else {
            -std::f64::consts::FRAC_PI_2
        };
        return Geodetic {
            lon,
            lat,
            height: position.z.abs() - WGS84_B,
        };
    }

    let mut lat = position.z.atan2(horizontal * (1.0 - WGS84_E2));
    let mut height = 0.0;
    for _ in 0..6 {
        let (sin_lat, cos_lat) = lat.sin_cos();
        let prime_vertical = WGS84_A / (1.0 - WGS84_E2 * sin_lat * sin_lat).sqrt();
        height = horizontal / cos_lat - prime_vertical;
        let next = position
            .z
            .atan2(horizontal * (1.0 - WGS84_E2 * prime_vertical / (prime_vertical + height)));
        if (next - lat).abs() < 1e-12 {
            lat = next;
            break;
        }
        lat = next;
    }

    Geodetic { lon, lat, height }
}

/// Outward ellipsoid normal at an ECEF position.
pub fn ellipsoid_normal(position: DVec3) -> DVec3 {
    DVec3::new(
        position.x / (WGS84_A * WGS84_A),
        position.y / (WGS84_A * WGS84_A),
        position.z / (WGS84_B * WGS84_B),
    )
    .normalize()
}

/// East/North/Up basis at an ECEF position, as (east, north, up).
pub fn enu_axes(position: DVec3) -> (DVec3, DVec3, DVec3) {
    let geodetic = ecef_to_geodetic(position);
    let (sin_lon, cos_lon) = geodetic.lon.sin_cos();
    let (sin_lat, cos_lat) = geodetic.lat.sin_cos();
    let east = DVec3::new(-sin_lon, cos_lon, 0.0);
    let north = DVec3::new(-sin_lat * cos_lon, -sin_lat * sin_lon, cos_lat);
    let up = DVec3::new(cos_lat * cos_lon, cos_lat * sin_lon, sin_lat);
    (east, north, up)
}

/// Scales ECEF into the unit-sphere space where the ellipsoid is a sphere.
fn to_unit_sphere(position: DVec3) -> DVec3 {
    DVec3::new(
        position.x / WGS84_A,
        position.y / WGS84_A,
        position.z / WGS84_B,
    )
}

/// Distance along a (not necessarily normalized) ray to the first ellipsoid
/// intersection, in the same units as `direction`.
pub fn ray_ellipsoid(origin: DVec3, direction: DVec3) -> Option<f64> {
    let origin = to_unit_sphere(origin);
    let direction = DVec3::new(
        direction.x / WGS84_A,
        direction.y / WGS84_A,
        direction.z / WGS84_B,
    );
    let a = direction.length_squared();
    if a <= f64::EPSILON {
        return None;
    }
    let b = 2.0 * origin.dot(direction);
    let c = origin.length_squared() - 1.0;
    let discriminant = b * b - 4.0 * a * c;
    if discriminant < 0.0 {
        return None;
    }
    let root = discriminant.sqrt();
    let t0 = (-b - root) / (2.0 * a);
    let t1 = (-b + root) / (2.0 * a);
    if t0 >= 0.0 {
        Some(t0)
    } else if t1 >= 0.0 {
        Some(t1)
    } else {
        None
    }
}

/// Both roots `(near, far)` of a ray against the ellipsoid grown by `height`
/// metres (both semi-axes), in units of `direction`; `None` on a miss.
pub fn ray_ellipsoid_span(origin: DVec3, direction: DVec3, height: f64) -> Option<(f64, f64)> {
    let (a_axis, b_axis) = (WGS84_A + height, WGS84_B + height);
    let origin = DVec3::new(origin.x / a_axis, origin.y / a_axis, origin.z / b_axis);
    let direction = DVec3::new(
        direction.x / a_axis,
        direction.y / a_axis,
        direction.z / b_axis,
    );
    let a = direction.length_squared();
    if a <= f64::EPSILON {
        return None;
    }
    let b = 2.0 * origin.dot(direction);
    let c = origin.length_squared() - 1.0;
    let discriminant = b * b - 4.0 * a * c;
    if discriminant < 0.0 {
        return None;
    }
    let root = discriminant.sqrt();
    Some(((-b - root) / (2.0 * a), (-b + root) / (2.0 * a)))
}

/// Steps taken along a ray through the terrain shell. A summit narrower than
/// the step can be missed at grazing angles; steeper views step in metres.
const TERRAIN_RAY_STEPS: u32 = 512;

/// First point where a ray meets the terrain surface `ground` describes
/// (heights above the ellipsoid, `None` meaning the ellipsoid itself), or
/// `None` when it flies past the globe.
///
/// The ray is marched through the shell between sea level and the highest
/// possible summit, then the crossing is bisected. A ray that starts below
/// the terrain returns its origin.
pub fn ray_terrain(origin: DVec3, direction: DVec3, ground: &GroundFn<'_>) -> Option<DVec3> {
    let length = direction.length();
    if !length.is_finite() || length <= f64::EPSILON {
        return None;
    }
    let direction = direction / length;
    let (shell_in, shell_out) =
        ray_ellipsoid_span(origin, direction, crate::terrain::MAX_TERRAIN_HEIGHT_M)?;
    if shell_out < 0.0 {
        return None;
    }
    let start = shell_in.max(0.0);
    let (end, hits_sea) = match ray_ellipsoid_span(origin, direction, 0.0) {
        Some((near, _)) if near >= start => (near, true),
        _ => (shell_out, false),
    };

    let clearance = |t: f64| {
        let geodetic = ecef_to_geodetic(origin + direction * t);
        geodetic.height - ground(geodetic.lon_deg(), geodetic.lat_deg()).unwrap_or(0.0)
    };
    if clearance(start) <= 0.0 {
        return Some(origin + direction * start);
    }
    let mut previous = start;
    for step in 1..=TERRAIN_RAY_STEPS {
        let t = start + (end - start) * f64::from(step) / f64::from(TERRAIN_RAY_STEPS);
        if clearance(t) <= 0.0 {
            let (mut above, mut below) = (previous, t);
            for _ in 0..32 {
                let middle = 0.5 * (above + below);
                if clearance(middle) <= 0.0 {
                    below = middle;
                } else {
                    above = middle;
                }
            }
            return Some(origin + direction * below);
        }
        previous = t;
    }
    hits_sea.then(|| origin + direction * end)
}

/// Intersects the ellipsoid with a ray and returns the ECEF hit point.
pub fn ray_ellipsoid_point(origin: DVec3, direction: DVec3) -> Option<DVec3> {
    ray_ellipsoid(origin, direction).map(|t| origin + direction * t)
}

/// Whether `target` is above the horizon as seen from `eye`.
///
/// Uses a segment/ellipsoid test, which is exact for the ellipsoid and does not
/// need a separate horizon-angle approximation.
pub fn is_point_visible(eye: DVec3, target: DVec3) -> bool {
    let eye_scaled = to_unit_sphere(eye);
    if eye_scaled.length() < 1.0 - 1e-9 {
        return true;
    }
    let target_scaled = to_unit_sphere(target);
    let direction = target_scaled - eye_scaled;
    let a = direction.length_squared();
    if a <= f64::EPSILON {
        return true;
    }
    let b = 2.0 * eye_scaled.dot(direction);
    let c = eye_scaled.length_squared() - 1.0;
    let discriminant = b * b - 4.0 * a * c;
    if discriminant < 0.0 {
        return true;
    }
    let root = discriminant.sqrt();
    let t0 = (-b - root) / (2.0 * a);
    let t1 = (-b + root) / (2.0 * a);

    // Endpoints are on (or near) the surface by construction, so only a
    // strictly interior crossing means the far side of the globe.
    const MARGIN: f64 = 1e-6;
    let crosses = |t: f64| t > MARGIN && t < 1.0 - MARGIN;
    !(crosses(t0) || crosses(t1))
}

/// Latitude/longitude (degrees) of a normalized Web-Mercator world position.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TileBounds {
    pub west: f64,
    pub south: f64,
    pub east: f64,
    pub north: f64,
}

impl TileBounds {
    pub fn center(&self) -> Geodetic {
        Geodetic::from_degrees(
            (self.west + self.east) / 2.0,
            (self.south + self.north) / 2.0,
            0.0,
        )
    }

    pub fn width_deg(&self) -> f64 {
        self.east - self.west
    }

    pub fn height_deg(&self) -> f64 {
        self.north - self.south
    }

    /// Longest side of the tile on the ground, in meters.
    ///
    /// Mercator tiles are square in the projection, not on the ellipsoid: away
    /// from the equator they get taller (by roughly `1 / cos(lat)`) while
    /// getting narrower. LOD and skirt depth have to use the longer side,
    /// otherwise polar tiles are treated as much smaller than they are.
    pub fn ground_size_m(&self) -> f64 {
        let mid_lat = (self.north + self.south).to_radians() * 0.5;
        let east_west = WGS84_A * self.width_deg().to_radians() * mid_lat.cos();
        let north_south = WGS84_A * self.height_deg().to_radians();
        east_west.max(north_south)
    }
}

/// Web-Mercator tile address.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TileId {
    pub z: u8,
    pub x: u32,
    pub y: u32,
}

impl std::fmt::Display for TileId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}/{}", self.z, self.x, self.y)
    }
}

impl TileId {
    pub fn new(z: u8, x: u32, y: u32) -> Option<Self> {
        let count = 1u32.checked_shl(u32::from(z))?;
        (x < count && y < count).then_some(Self { z, x, y })
    }

    pub fn root() -> Self {
        Self { z: 0, x: 0, y: 0 }
    }

    pub fn children(self) -> [Option<TileId>; 4] {
        let z = self.z + 1;
        let x = self.x * 2;
        let y = self.y * 2;
        [
            TileId::new(z, x, y),
            TileId::new(z, x + 1, y),
            TileId::new(z, x, y + 1),
            TileId::new(z, x + 1, y + 1),
        ]
    }

    pub fn parent(self) -> Option<TileId> {
        (self.z > 0).then(|| TileId {
            z: self.z - 1,
            x: self.x / 2,
            y: self.y / 2,
        })
    }

    pub fn bounds(self) -> TileBounds {
        let count = 2f64.powi(i32::from(self.z));
        let lon = |x: u32| f64::from(x) / count * 360.0 - 180.0;
        let lat = |y: u32| {
            let n = std::f64::consts::PI * (1.0 - 2.0 * f64::from(y) / count);
            n.sinh().atan().to_degrees()
        };
        TileBounds {
            west: lon(self.x),
            east: lon(self.x + 1),
            south: lat(self.y + 1),
            north: lat(self.y),
        }
    }

    /// Approximate ground size of the tile at the equator, in meters.
    pub fn equator_size_m(self) -> f64 {
        2.0 * std::f64::consts::PI * WGS84_A / 2f64.powi(i32::from(self.z))
    }

    /// Longest side of the tile on the ground, in meters.
    pub fn ground_size_m(self) -> f64 {
        self.bounds().ground_size_m()
    }
}

/// Normalized Web-Mercator position of a lon/lat pair.
pub fn lon_lat_to_mercator_uv(lon_deg: f64, lat_deg: f64) -> (f64, f64) {
    let lat = lat_deg
        .clamp(-MERCATOR_MAX_LAT_DEG, MERCATOR_MAX_LAT_DEG)
        .to_radians();
    let u = (lon_deg + 180.0) / 360.0;
    let v = 0.5 - (lat.tan() + 1.0 / lat.cos()).ln() / (2.0 * std::f64::consts::PI);
    (u, v)
}

/// Inverse of [`lon_lat_to_mercator_uv`].
pub fn mercator_uv_to_lon_lat(u: f64, v: f64) -> (f64, f64) {
    let lon = u * 360.0 - 180.0;
    let n = std::f64::consts::PI * (1.0 - 2.0 * v);
    let lat = n.sinh().atan().to_degrees();
    (lon, lat)
}

/// UV rectangle (u0, v0, du, dv) of `tile` inside `ancestor`'s texture.
pub fn tile_uv_rect(tile: TileId, ancestor: TileId) -> [f32; 4] {
    let depth = tile.z.saturating_sub(ancestor.z);
    let scale = 2u32.pow(u32::from(depth));
    let offset_x = tile.x - ancestor.x * scale;
    let offset_y = tile.y - ancestor.y * scale;
    let size = 1.0 / f32::from(u16::try_from(scale).unwrap_or(u16::MAX));
    [
        f32::from(u16::try_from(offset_x).unwrap_or(0)) * size,
        f32::from(u16::try_from(offset_y).unwrap_or(0)) * size,
        size,
        size,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_core::{AltitudeMode, GeoPoint};

    fn at(altitude: Option<f64>, mode: Option<AltitudeMode>) -> GeoPoint {
        GeoPoint {
            lon: 9.9,
            lat: 51.1,
            altitude,
            altitude_mode: mode,
        }
    }

    const MODES: [Option<AltitudeMode>; 4] = [
        None,
        Some(AltitudeMode::ClampToGround),
        Some(AltitudeMode::RelativeToGround),
        Some(AltitudeMode::Absolute),
    ];

    #[test]
    fn without_terrain_every_mode_is_clamped_to_the_surface() {
        for mode in MODES {
            for altitude in [None, Some(492.5), Some(-30.0)] {
                assert_eq!(render_height(&at(altitude, mode), None), 0.0);
            }
        }
    }

    #[test]
    fn with_terrain_the_altitude_mode_decides() {
        let ground = Some(200.0);
        assert_eq!(render_height(&at(Some(492.5), None), ground), 200.0);
        let clamp = Some(AltitudeMode::ClampToGround);
        assert_eq!(render_height(&at(Some(492.5), clamp), ground), 200.0);
        let relative = Some(AltitudeMode::RelativeToGround);
        assert_eq!(render_height(&at(Some(50.0), relative), ground), 250.0);
        assert_eq!(render_height(&at(Some(-30.0), relative), ground), 170.0);
        assert_eq!(render_height(&at(None, relative), ground), 200.0);
        let absolute = Some(AltitudeMode::Absolute);
        assert_eq!(render_height(&at(Some(492.5), absolute), ground), 492.5);
        assert_eq!(render_height(&at(Some(-30.0), absolute), ground), -30.0);
        assert_eq!(render_height(&at(None, absolute), ground), 200.0);
    }

    #[test]
    fn ecef_conversion_ignores_altitude_without_terrain() {
        let high = at(Some(5000.0), None);
        let low = at(None, None);
        assert_eq!(
            geo_point_to_ecef(&high, &no_ground),
            geo_point_to_ecef(&low, &no_ground)
        );
        let lifted = geo_point_to_ecef(&at(Some(5000.0), Some(AltitudeMode::Absolute)), &|_, _| {
            Some(100.0)
        });
        assert!((ecef_to_geodetic(lifted).height - 5000.0).abs() < 0.01);
    }

    fn close(a: DVec3, b: DVec3, tolerance: f64) -> bool {
        (a - b).length() < tolerance
    }

    #[test]
    fn ecef_round_trips_for_known_points() {
        for (lon, lat, height) in [
            (0.0, 0.0, 0.0),
            (13.405, 52.52, 250.0),
            (-70.5, -33.45, 0.0),
            (179.999, 85.0, -400.0),
            (-179.999, -85.0, 8000.0),
        ] {
            let original = Geodetic::from_degrees(lon, lat, height);
            let ecef = geodetic_to_ecef(original);
            let restored = ecef_to_geodetic(ecef);
            assert!(
                (restored.lon_deg() - lon).abs() < 1e-9,
                "lon {lon} -> {}",
                restored.lon_deg()
            );
            assert!(
                (restored.lat_deg() - lat).abs() < 1e-9,
                "lat {lat} -> {}",
                restored.lat_deg()
            );
            assert!(
                (restored.height - height).abs() < 1e-4,
                "height {height} -> {}",
                restored.height
            );
        }
    }

    #[test]
    fn equator_point_has_expected_ecef() {
        let point = geodetic_to_ecef(Geodetic::from_degrees(0.0, 0.0, 0.0));
        assert!(close(point, DVec3::new(WGS84_A, 0.0, 0.0), 1e-6));
        let north_pole = geodetic_to_ecef(Geodetic::from_degrees(0.0, 90.0, 0.0));
        assert!(close(north_pole, DVec3::new(0.0, 0.0, WGS84_B), 1e-6));
    }

    #[test]
    fn ray_from_space_hits_the_subpoint() {
        let target = geodetic_to_ecef(Geodetic::from_degrees(10.0, 20.0, 0.0));
        let eye = target.normalize() * (WGS84_A + 2_000_000.0);
        let hit = ray_ellipsoid_point(eye, target - eye).expect("ray must hit");
        let hit_geodetic = ecef_to_geodetic(hit);
        assert!((hit_geodetic.lon_deg() - 10.0).abs() < 1e-6);
        assert!((hit_geodetic.lat_deg() - 20.0).abs() < 1e-6);
    }

    fn height_of(point: DVec3) -> f64 {
        ecef_to_geodetic(point).height
    }

    #[test]
    fn terrain_ray_hits_a_plateau_instead_of_the_ellipsoid() {
        let target = geodetic_to_ecef(Geodetic::from_degrees(10.0, 20.0, 0.0));
        let eye = target.normalize() * (WGS84_A + 50_000.0);
        let plateau = |_: f64, _: f64| Some(1_000.0);
        let hit = ray_terrain(eye, target - eye, &plateau).expect("hits the plateau");
        assert!(
            (height_of(hit) - 1_000.0).abs() < 0.01,
            "{}",
            height_of(hit)
        );
        let sea = ray_terrain(eye, target - eye, &no_ground).expect("hits the sea level");
        assert!(height_of(sea).abs() < 0.01);
        assert!((hit - eye).length() < (sea - eye).length());
    }

    #[test]
    fn terrain_ray_lands_on_a_mountain_between_the_eye_and_the_ellipsoid() {
        // A 3 km cone of 20 km radius around (10, 20).
        let cone = |lon: f64, lat: f64| {
            let d = ((lon - 10.0).hypot(lat - 20.0)).to_radians() * WGS84_A;
            Some((3_000.0 - d * 0.15).max(0.0))
        };
        let summit = geodetic_to_ecef(Geodetic::from_degrees(10.0, 20.0, 3_000.0));
        let eye = geodetic_to_ecef(Geodetic::from_degrees(10.0, 20.0, 40_000.0));
        let hit = ray_terrain(eye, summit - eye, &cone).expect("hits");
        assert!((height_of(hit) - 3_000.0).abs() < 0.5, "{}", height_of(hit));
    }

    #[test]
    fn terrain_ray_that_misses_the_globe_returns_none() {
        let eye = DVec3::new(WGS84_A + 1_000_000.0, 0.0, 0.0);
        let plateau = |_: f64, _: f64| Some(1_000.0);
        assert!(ray_terrain(eye, DVec3::new(0.0, 1.0, 0.0), &plateau).is_none());
        assert!(ray_terrain(eye, DVec3::ZERO, &plateau).is_none());
        assert!(ray_terrain(eye, DVec3::new(1.0, 0.0, 0.0), &plateau).is_none());
    }

    #[test]
    fn ray_missing_the_globe_returns_none() {
        let eye = DVec3::new(WGS84_A + 1_000_000.0, 0.0, 0.0);
        assert!(ray_ellipsoid(eye, DVec3::new(0.0, 1.0, 0.0)).is_none());
    }

    #[test]
    fn horizon_hides_the_far_side() {
        let eye = geodetic_to_ecef(Geodetic::from_degrees(0.0, 0.0, 500_000.0));
        let near = geodetic_to_ecef(Geodetic::from_degrees(1.0, 0.0, 0.0));
        let far = geodetic_to_ecef(Geodetic::from_degrees(179.0, 0.0, 0.0));
        assert!(is_point_visible(eye, near));
        assert!(!is_point_visible(eye, far));
    }

    #[test]
    fn low_camera_still_sees_the_nearby_surface() {
        let eye = geodetic_to_ecef(Geodetic::from_degrees(13.405, 52.52, 500.0));
        let target = geodetic_to_ecef(Geodetic::from_degrees(13.41, 52.521, 0.0));
        assert!(is_point_visible(eye, target));
    }

    #[test]
    fn mercator_tile_bounds_are_correct() {
        let root = TileId::root().bounds();
        assert!((root.west + 180.0).abs() < 1e-9);
        assert!((root.east - 180.0).abs() < 1e-9);
        assert!((root.north - MERCATOR_MAX_LAT_DEG).abs() < 1e-9);
        assert!((root.south + MERCATOR_MAX_LAT_DEG).abs() < 1e-9);

        let north_east = TileId::new(1, 1, 0).expect("valid tile").bounds();
        assert!((north_east.west - 0.0).abs() < 1e-9);
        assert!((north_east.east - 180.0).abs() < 1e-9);
        assert!((north_east.south - 0.0).abs() < 1e-9);
    }

    #[test]
    fn mercator_uv_round_trips() {
        for (lon, lat) in [(0.0, 0.0), (13.405, 52.52), (-170.0, -60.0)] {
            let (u, v) = lon_lat_to_mercator_uv(lon, lat);
            let (lon2, lat2) = mercator_uv_to_lon_lat(u, v);
            assert!((lon2 - lon).abs() < 1e-9);
            assert!((lat2 - lat).abs() < 1e-9);
        }
    }

    #[test]
    fn child_uv_rect_is_relative_to_the_ancestor() {
        let ancestor = TileId::new(2, 1, 1).expect("valid");
        assert_eq!(tile_uv_rect(ancestor, ancestor), [0.0, 0.0, 1.0, 1.0]);
        let child = TileId::new(3, 3, 3).expect("valid");
        assert_eq!(tile_uv_rect(child, ancestor), [0.5, 0.5, 0.5, 0.5]);
    }

    #[test]
    fn tile_parent_and_children_are_consistent() {
        let tile = TileId::new(4, 9, 6).expect("valid");
        let parent = tile.parent().expect("has parent");
        assert_eq!(parent, TileId::new(3, 4, 3).expect("valid"));
        let children = parent.children();
        assert!(children.iter().flatten().any(|child| *child == tile));
        assert!(TileId::new(1, 2, 0).is_none());
    }
}
