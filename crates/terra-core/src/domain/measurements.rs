//! Geometric measurements shared by the editor, ruler and height profile.
//!
//! Ported from `geometryMeasurements.ts`, `geometryCenter.ts` and
//! `trackProfile.ts`; the formulas match so numbers agree with the web app.

use crate::domain::geometry::{GeoPoint, PlacemarkGeometry};

/// Mean Earth radius used by the web app for spherical shortcuts.
pub const EARTH_RADIUS_METERS: f64 = 6_371_000.0;

fn to_radians(degrees: f64) -> f64 {
    degrees * std::f64::consts::PI / 180.0
}

pub fn haversine_meters(a: &GeoPoint, b: &GeoPoint) -> f64 {
    let d_lat = to_radians(b.lat - a.lat);
    let d_lon = to_radians(b.lon - a.lon);
    let sin_lat = (d_lat / 2.0).sin();
    let sin_lon = (d_lon / 2.0).sin();
    let h =
        sin_lat * sin_lat + to_radians(a.lat).cos() * to_radians(b.lat).cos() * sin_lon * sin_lon;
    2.0 * EARTH_RADIUS_METERS * h.sqrt().min(1.0).asin()
}

pub fn geometry_center(geometry: &PlacemarkGeometry) -> GeoPoint {
    match geometry {
        PlacemarkGeometry::Point { coordinates } => *coordinates,
        PlacemarkGeometry::Circle { center, .. } => *center,
        PlacemarkGeometry::Rectangle {
            north,
            south,
            east,
            west,
        } => GeoPoint::new((east + west) / 2.0, (north + south) / 2.0).unwrap_or(GeoPoint {
            lon: 0.0,
            lat: 0.0,
            altitude: None,
            altitude_mode: None,
        }),
        PlacemarkGeometry::Polygon { outer_ring, .. } => average_point(outer_ring),
        PlacemarkGeometry::LineString { path, .. } => average_point(path),
        PlacemarkGeometry::GroundOverlay { bounds, .. } => GeoPoint::new(
            (bounds.east + bounds.west) / 2.0,
            (bounds.north + bounds.south) / 2.0,
        )
        .unwrap_or(GeoPoint {
            lon: 0.0,
            lat: 0.0,
            altitude: None,
            altitude_mode: None,
        }),
        PlacemarkGeometry::Model { position, .. } => *position,
    }
}

fn average_point(points: &[GeoPoint]) -> GeoPoint {
    if points.is_empty() {
        return GeoPoint {
            lon: 0.0,
            lat: 0.0,
            altitude: None,
            altitude_mode: None,
        };
    }
    let count = points.len() as f64;
    let lon = points.iter().map(|point| point.lon).sum::<f64>() / count;
    let lat = points.iter().map(|point| point.lat).sum::<f64>() / count;
    GeoPoint::new(lon, lat).unwrap_or(GeoPoint {
        lon: 0.0,
        lat: 0.0,
        altitude: None,
        altitude_mode: None,
    })
}

pub fn circle_area_square_meters(radius_meters: f64) -> f64 {
    std::f64::consts::PI * radius_meters * radius_meters
}

pub fn circle_circumference_meters(radius_meters: f64) -> f64 {
    2.0 * std::f64::consts::PI * radius_meters
}

/// Exact area of a graticule rectangle (bounded by meridians and parallels).
pub fn rectangle_area_square_meters(north: f64, south: f64, east: f64, west: f64) -> f64 {
    let lon_span = to_radians(east - west);
    let lat_factor = to_radians(north).sin() - to_radians(south).sin();
    (EARTH_RADIUS_METERS * EARTH_RADIUS_METERS * lon_span * lat_factor).abs()
}

fn ring_area_square_meters(ring: &[GeoPoint]) -> f64 {
    if ring.len() < 3 {
        return 0.0;
    }
    let mut sum = 0.0;
    for index in 0..ring.len() {
        let a = &ring[index];
        let b = &ring[(index + 1) % ring.len()];
        sum +=
            to_radians(b.lon - a.lon) * (2.0 + to_radians(a.lat).sin() + to_radians(b.lat).sin());
    }
    (sum * EARTH_RADIUS_METERS * EARTH_RADIUS_METERS / 2.0).abs()
}

pub fn polygon_area_square_meters(outer: &[GeoPoint], inner: &[Vec<GeoPoint>]) -> f64 {
    let holes: f64 = inner.iter().map(|ring| ring_area_square_meters(ring)).sum();
    (ring_area_square_meters(outer) - holes).max(0.0)
}

/// The numbers the placemark editor shows, depending on the geometry type.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Measurements {
    pub area_square_meters: Option<f64>,
    pub radius_meters: Option<f64>,
    pub circumference_meters: Option<f64>,
    pub center: Option<GeoPoint>,
}

pub fn measurements(geometry: &PlacemarkGeometry) -> Measurements {
    let center = geometry_center(geometry);
    match geometry {
        PlacemarkGeometry::Circle { radius_meters, .. } => Measurements {
            area_square_meters: Some(circle_area_square_meters(*radius_meters)),
            radius_meters: Some(*radius_meters),
            circumference_meters: Some(circle_circumference_meters(*radius_meters)),
            center: Some(center),
        },
        PlacemarkGeometry::Rectangle {
            north,
            south,
            east,
            west,
        } => Measurements {
            area_square_meters: Some(rectangle_area_square_meters(*north, *south, *east, *west)),
            center: Some(center),
            ..Default::default()
        },
        PlacemarkGeometry::Polygon {
            outer_ring,
            inner_rings,
            ..
        } => Measurements {
            area_square_meters: Some(polygon_area_square_meters(
                outer_ring,
                inner_rings.as_deref().unwrap_or(&[]),
            )),
            center: Some(center),
            ..Default::default()
        },
        PlacemarkGeometry::LineString { path, .. } => Measurements {
            circumference_meters: Some(path_length_meters(path)),
            center: Some(center),
            ..Default::default()
        },
        _ => Measurements {
            center: Some(center),
            ..Default::default()
        },
    }
}

pub fn path_length_meters(path: &[GeoPoint]) -> f64 {
    path.windows(2)
        .map(|pair| haversine_meters(&pair[0], &pair[1]))
        .sum()
}

/// Half the largest dimension, used to size a fly-to.
pub fn geometry_extent_meters(geometry: &PlacemarkGeometry) -> f64 {
    let center = geometry_center(geometry);
    match geometry {
        PlacemarkGeometry::Circle { radius_meters, .. } => *radius_meters,
        PlacemarkGeometry::Point { .. } | PlacemarkGeometry::Model { .. } => 500.0,
        PlacemarkGeometry::Rectangle {
            north,
            south,
            east,
            west,
        } => {
            let diagonal = haversine_meters(
                &GeoPoint {
                    lon: *west,
                    lat: *south,
                    altitude: None,
                    altitude_mode: None,
                },
                &GeoPoint {
                    lon: *east,
                    lat: *north,
                    altitude: None,
                    altitude_mode: None,
                },
            );
            diagonal / 2.0
        }
        PlacemarkGeometry::Polygon { outer_ring, .. } => outer_ring
            .iter()
            .map(|point| haversine_meters(&center, point))
            .fold(0.0, f64::max)
            .max(100.0),
        PlacemarkGeometry::LineString { path, .. } => path
            .iter()
            .map(|point| haversine_meters(&center, point))
            .fold(0.0, f64::max)
            .max(100.0),
        PlacemarkGeometry::GroundOverlay { bounds, .. } => {
            haversine_meters(
                &GeoPoint {
                    lon: bounds.west,
                    lat: bounds.south,
                    altitude: None,
                    altitude_mode: None,
                },
                &GeoPoint {
                    lon: bounds.east,
                    lat: bounds.north,
                    altitude: None,
                    altitude_mode: None,
                },
            ) / 2.0
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrackProfilePoint {
    pub distance_meters: f64,
    pub altitude_meters: f64,
}

pub fn compute_track_profile(path: &[GeoPoint]) -> Vec<TrackProfilePoint> {
    let mut distance = 0.0;
    path.iter()
        .enumerate()
        .map(|(index, point)| {
            if index > 0 {
                distance += haversine_meters(&path[index - 1], point);
            }
            TrackProfilePoint {
                distance_meters: distance,
                altitude_meters: point.altitude.unwrap_or(0.0),
            }
        })
        .collect()
}

pub fn has_elevation_data(geometry: &PlacemarkGeometry) -> bool {
    match geometry {
        PlacemarkGeometry::LineString { path, .. } => {
            path.iter().any(|point| point.altitude.is_some())
        }
        _ => false,
    }
}

pub fn pick_grid_step_meters(total_distance_meters: f64) -> f64 {
    if total_distance_meters < 5000.0 {
        500.0
    } else {
        1000.0
    }
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
    fn haversine_matches_a_degree_of_latitude() {
        let distance = haversine_meters(&point(0.0, 0.0), &point(0.0, 1.0));
        assert!((distance - 111_195.0).abs() < 200.0, "got {distance}");
    }

    #[test]
    fn circle_measurements_match_the_formulas() {
        let geometry = PlacemarkGeometry::circle(point(0.0, 0.0), 1000.0).expect("circle");
        let result = measurements(&geometry);
        assert!(
            (result.area_square_meters.unwrap() - std::f64::consts::PI * 1_000_000.0).abs() < 1.0
        );
        assert!(
            (result.circumference_meters.unwrap() - 2.0 * std::f64::consts::PI * 1000.0).abs()
                < 1e-6
        );
        assert!((result.radius_meters.unwrap() - 1000.0).abs() < 1e-9);
    }

    #[test]
    fn rectangle_area_shrinks_towards_the_poles() {
        let equator = rectangle_area_square_meters(1.0, 0.0, 1.0, 0.0);
        let high = rectangle_area_square_meters(81.0, 80.0, 1.0, 0.0);
        assert!(high < equator);
        assert!(equator > 1.0e10 && equator < 1.3e10);
    }

    #[test]
    fn rectangle_area_agrees_with_the_polygon_formula() {
        let as_rectangle = rectangle_area_square_meters(10.0, 0.0, 10.0, 0.0);
        let ring = [
            point(0.0, 0.0),
            point(10.0, 0.0),
            point(10.0, 10.0),
            point(0.0, 10.0),
        ];
        let as_polygon = polygon_area_square_meters(&ring, &[]);
        assert!(
            (as_rectangle - as_polygon).abs() / as_rectangle < 0.01,
            "rectangle {as_rectangle} vs polygon {as_polygon}"
        );
    }

    #[test]
    fn polygon_holes_are_subtracted() {
        let outer = [
            point(0.0, 0.0),
            point(10.0, 0.0),
            point(10.0, 10.0),
            point(0.0, 10.0),
        ];
        let hole = [
            point(4.0, 4.0),
            point(6.0, 4.0),
            point(6.0, 6.0),
            point(4.0, 6.0),
        ];
        let solid = polygon_area_square_meters(&outer, &[]);
        let with_hole = polygon_area_square_meters(&outer, &[hole.to_vec()]);
        assert!(with_hole < solid);
        assert!(with_hole > 0.0);
    }

    #[test]
    fn degenerate_polygon_has_no_area() {
        let ring = [point(0.0, 0.0), point(1.0, 0.0), point(0.5, 0.0)];
        assert!(polygon_area_square_meters(&ring, &[]) < 1.0);
    }

    #[test]
    fn center_and_extent_are_available_for_every_shape() {
        for geometry in [
            PlacemarkGeometry::point(point(1.0, 2.0)).expect("point"),
            PlacemarkGeometry::circle(point(1.0, 2.0), 500.0).expect("circle"),
            PlacemarkGeometry::rectangle(
                crate::domain::geometry::RectangleBounds::new(3.0, 2.0, 2.0, 1.0).expect("bounds"),
            )
            .expect("rectangle"),
            PlacemarkGeometry::line_string(vec![point(1.0, 2.0), point(1.5, 2.5)], None, None)
                .expect("line"),
        ] {
            assert!(geometry_extent_meters(&geometry) > 0.0);
            assert!(measurements(&geometry).center.is_some());
        }
    }

    #[test]
    fn track_profile_accumulates_distance() {
        let path = vec![point(0.0, 0.0), point(0.0, 1.0), point(0.0, 2.0)];
        let profile = compute_track_profile(&path);
        assert_eq!(profile.len(), 3);
        assert_eq!(profile[0].distance_meters, 0.0);
        assert!((profile[2].distance_meters - 2.0 * profile[1].distance_meters).abs() < 1.0);
        assert_eq!(pick_grid_step_meters(4000.0), 500.0);
        assert_eq!(pick_grid_step_meters(6000.0), 1000.0);
    }
}
