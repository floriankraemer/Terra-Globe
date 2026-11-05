//! Circle approximation as a polygon ring.
//!
//! KML has no native circle geometry and the renderer tessellates circles as
//! polygons, so both the exporter and the globe use this one approximation.

use crate::domain::geometry::GeoPoint;

const EARTH_RADIUS_METERS: f64 = 6_371_000.0;
pub const DEFAULT_SEGMENTS: usize = 64;

/// Closed ring around `center` at `radius_meters`, using the spherical
/// destination-point formula the web app writes.
///
/// Longitudes inside the valid range are left untouched (so exported files match
/// the web app bit for bit); only rings crossing the antimeridian are wrapped.
pub fn circle_to_polygon_ring(
    center: &GeoPoint,
    radius_meters: f64,
    segments: usize,
) -> Vec<GeoPoint> {
    let segments = segments.max(3);
    let lat1 = to_radians(center.lat);
    let lon1 = to_radians(center.lon);
    let angular_distance = radius_meters / EARTH_RADIUS_METERS;

    let mut ring = Vec::with_capacity(segments + 1);
    for index in 0..=segments {
        let bearing = std::f64::consts::TAU * index as f64 / segments as f64;
        let lat2 = (lat1.sin() * angular_distance.cos()
            + lat1.cos() * angular_distance.sin() * bearing.cos())
        .asin();
        let lon2 = lon1
            + (bearing.sin() * angular_distance.sin() * lat1.cos())
                .atan2(angular_distance.cos() - lat1.sin() * lat2.sin());
        ring.push(GeoPoint {
            lon: normalize_longitude(to_degrees(lon2)),
            lat: to_degrees(lat2).clamp(-90.0, 90.0),
            altitude: center.altitude,
            altitude_mode: center.altitude_mode,
        });
    }
    ring
}

/// Same operation order as the JavaScript implementation, so exported rings
/// match it bit for bit (a precomputed `PI / 180` constant differs in the last
/// ULP).
fn to_radians(degrees: f64) -> f64 {
    degrees * std::f64::consts::PI / 180.0
}

fn to_degrees(radians: f64) -> f64 {
    radians * 180.0 / std::f64::consts::PI
}

fn normalize_longitude(degrees: f64) -> f64 {
    if (-180.0..=180.0).contains(&degrees) {
        return degrees;
    }
    let value = (degrees + 180.0).rem_euclid(360.0) - 180.0;
    if value == -180.0 { 180.0 } else { value }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(lon: f64, lat: f64) -> GeoPoint {
        GeoPoint {
            lon,
            lat,
            altitude: None,
            altitude_mode: None,
        }
    }

    #[test]
    fn ring_is_closed_and_has_one_vertex_per_segment_plus_the_close() {
        let ring = circle_to_polygon_ring(&point(13.405, 52.52), 1_000.0, 64);
        assert_eq!(ring.len(), 65);
        let first = ring.first().expect("first");
        let last = ring.last().expect("last");
        assert!((first.lon - last.lon).abs() < 1e-9);
        assert!((first.lat - last.lat).abs() < 1e-9);
        for vertex in &ring {
            vertex.validate().expect("ring vertices are valid");
        }
    }

    #[test]
    fn radius_is_approximated_within_a_percent() {
        let center = point(0.0, 0.0);
        let radius = 10_000.0;
        let ring = circle_to_polygon_ring(&center, radius, 720);
        for vertex in &ring[..720] {
            let lat1 = to_radians(center.lat);
            let lat2 = to_radians(vertex.lat);
            let dlon = to_radians(vertex.lon - center.lon);
            let a = ((lat2 - lat1) / 2.0).sin().powi(2)
                + lat1.cos() * lat2.cos() * (dlon / 2.0).sin().powi(2);
            let distance = 2.0 * EARTH_RADIUS_METERS * a.sqrt().asin();
            assert!(
                (distance - radius).abs() < radius * 0.01,
                "distance {distance} vs radius {radius}"
            );
        }
    }

    #[test]
    fn rings_crossing_the_antimeridian_keep_valid_longitudes() {
        let ring = circle_to_polygon_ring(&point(179.99, 10.0), 5_000.0, 32);
        assert!(
            ring.iter()
                .all(|vertex| (-180.0..=180.0).contains(&vertex.lon))
        );
        assert!(ring.iter().any(|vertex| vertex.lon < 0.0));
    }
}
