//! Straight-line travel estimates for modes with no routing API (train, plane).
//!
//! Ported from `routeEstimate.ts`: a rough estimate drawn as a straight line
//! through the waypoints, not a real route.

use crate::domain::geometry::GeoPoint;
use crate::domain::measurements::haversine_meters;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EstimatedTravelMode {
    Train,
    Plane,
}

impl EstimatedTravelMode {
    pub fn meters_per_second(self) -> f64 {
        match self {
            Self::Train => 120_000.0 / 3600.0,
            Self::Plane => 800_000.0 / 3600.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct StraightLineRoute {
    pub path: Vec<GeoPoint>,
    pub distance_meters: f64,
    pub duration_seconds: f64,
}

pub fn estimate_straight_line_route(
    waypoints: &[GeoPoint],
    mode: EstimatedTravelMode,
) -> StraightLineRoute {
    let distance_meters: f64 = waypoints
        .windows(2)
        .map(|pair| haversine_meters(&pair[0], &pair[1]))
        .sum();
    StraightLineRoute {
        path: waypoints.to_vec(),
        distance_meters,
        duration_seconds: distance_meters / mode.meters_per_second(),
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
    fn sums_segment_distances_and_uses_mode_speed() {
        let waypoints = vec![point(0.0, 0.0), point(0.0, 1.0), point(0.0, 2.0)];
        let train = estimate_straight_line_route(&waypoints, EstimatedTravelMode::Train);
        assert_eq!(train.path.len(), 3);
        assert!(train.distance_meters > 222_000.0);

        let plane = estimate_straight_line_route(&waypoints, EstimatedTravelMode::Plane);
        assert!((plane.distance_meters - train.distance_meters).abs() < 1e-9);
        assert!(plane.duration_seconds < train.duration_seconds);
    }
}
