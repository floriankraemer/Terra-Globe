//! Ruler, route planner and the background jobs that talk to providers.

use std::sync::mpsc::{Receiver, Sender};

use terra_core::{
    EstimatedTravelMode, GeoPoint, Measurements, StraightLineRoute, estimate_straight_line_route,
    measurements,
};
#[cfg(not(target_arch = "wasm32"))]
use terra_providers::RoutingPreset;
use terra_providers::{GeocodeResult, RouteLeg, RoutingProfile};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteMode {
    Car,
    Foot,
    Bike,
    Plane,
    Train,
}

impl RouteMode {
    pub fn label_key(self) -> &'static str {
        match self {
            Self::Car => "routePlanner.modeCar",
            Self::Foot => "routePlanner.modeFoot",
            Self::Bike => "routePlanner.modeBike",
            Self::Plane => "routePlanner.modePlane",
            Self::Train => "routePlanner.modeTrain",
        }
    }

    pub fn routing_profile(self) -> Option<RoutingProfile> {
        match self {
            Self::Car => Some(RoutingProfile::Car),
            Self::Foot => Some(RoutingProfile::Foot),
            Self::Bike => Some(RoutingProfile::Bike),
            Self::Plane | Self::Train => None,
        }
    }

    pub fn estimate_mode(self) -> Option<EstimatedTravelMode> {
        match self {
            Self::Plane => Some(EstimatedTravelMode::Plane),
            Self::Train => Some(EstimatedTravelMode::Train),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RulerPhase {
    Idle,
    Measuring,
}

#[derive(Debug, Clone, Default)]
pub struct RulerState {
    pub points: Vec<GeoPoint>,
    pub finished: bool,
}

impl RulerState {
    pub fn is_active(&self) -> bool {
        !self.points.is_empty() && !self.finished
    }

    pub fn total_meters(&self) -> f64 {
        terra_core::path_length_meters(&self.points)
    }

    pub fn segment_meters(&self) -> Vec<f64> {
        self.points
            .windows(2)
            .map(|pair| terra_core::haversine_meters(&pair[0], &pair[1]))
            .collect()
    }

    pub fn add(&mut self, point: GeoPoint) {
        self.points.push(point);
        self.finished = false;
    }

    pub fn undo(&mut self) {
        self.points.pop();
    }

    pub fn finish(&mut self) {
        if self.points.len() >= 2 {
            self.finished = true;
        }
    }

    pub fn clear(&mut self) {
        self.points.clear();
        self.finished = false;
    }
}

#[derive(Debug, Clone, Default)]
pub struct RouteState {
    pub waypoints: Vec<GeoPoint>,
    pub mode: Option<RouteMode>,
    pub legs: Vec<RouteLeg>,
    pub error: Option<String>,
    pub pending: bool,
}

impl RouteState {
    pub fn is_active(&self) -> bool {
        !self.waypoints.is_empty() || !self.legs.is_empty()
    }

    pub fn clear(&mut self) {
        self.waypoints.clear();
        self.legs.clear();
        self.error = None;
    }

    pub fn route_path(&self) -> Option<Vec<GeoPoint>> {
        let leg = self.legs.first()?;
        Some(leg.path.clone())
    }

    pub fn total_distance_meters(&self) -> f64 {
        self.legs.iter().map(|leg| leg.distance_meters).sum()
    }

    pub fn total_duration_seconds(&self) -> f64 {
        self.legs.iter().map(|leg| leg.duration_seconds).sum()
    }
}

/// A straight-line estimate for plane/train, produced without the network.
pub fn estimate_route(waypoints: &[GeoPoint], mode: RouteMode) -> Option<StraightLineRoute> {
    let estimate = mode.estimate_mode()?;
    Some(estimate_straight_line_route(waypoints, estimate))
}

/// Work handed to a background thread so HTTP never blocks the UI.
#[derive(Debug)]
pub enum JobRequest {
    Geocode {
        preset: String,
        api_key: Option<String>,
        query: String,
    },
    Route {
        preset: String,
        api_key: Option<String>,
        waypoints: Vec<GeoPoint>,
        mode: RouteMode,
    },
}

#[derive(Debug)]
pub enum JobResult {
    Geocode(Result<Vec<GeocodeResult>, String>),
    Route(Result<Vec<RouteLeg>, String>),
}

/// Spawns provider work on a thread and delivers the result through a channel.
pub struct JobRunner {
    sender: Sender<JobResult>,
    receiver: Receiver<JobResult>,
}

impl Default for JobRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl JobRunner {
    pub fn new() -> Self {
        let (sender, receiver) = std::sync::mpsc::channel();
        Self { sender, receiver }
    }

    pub fn spawn(&self, request: JobRequest) {
        // Browsers have no threads here, and the provider clients are blocking,
        // so the browser build reports that network providers are unavailable
        // instead of panicking. Async fetch lands with the browser E2E pass.
        #[cfg(target_arch = "wasm32")]
        {
            let message =
                "network providers are not available in the browser build yet".to_string();
            let result = match request {
                JobRequest::Geocode { .. } => JobResult::Geocode(Err(message)),
                JobRequest::Route { .. } => JobResult::Route(Err(message)),
            };
            let _ = self.sender.send(result);
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let sender = self.sender.clone();
            std::thread::spawn(move || {
                let result = match request {
                    JobRequest::Geocode {
                        preset,
                        api_key,
                        query,
                    } => JobResult::Geocode(run_geocode(&preset, api_key, &query)),
                    JobRequest::Route {
                        preset,
                        api_key,
                        waypoints,
                        mode,
                    } => JobResult::Route(run_route(&preset, api_key, &waypoints, mode)),
                };
                let _ = sender.send(result);
            });
        }
    }

    pub fn poll(&self) -> Vec<JobResult> {
        let mut results = Vec::new();
        while let Ok(result) = self.receiver.try_recv() {
            results.push(result);
        }
        results
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn run_geocode(
    preset: &str,
    api_key: Option<String>,
    query: &str,
) -> Result<Vec<GeocodeResult>, String> {
    let client = std::sync::Arc::new(terra_providers::EhttpClient);
    let preset = match preset {
        "locationiq" => terra_providers::GeocodingPreset::LocationIq,
        "mapbox-geocoding" => terra_providers::GeocodingPreset::Mapbox,
        "opencage" => terra_providers::GeocodingPreset::OpenCage,
        _ => terra_providers::GeocodingPreset::Nominatim,
    };
    let provider = terra_providers::create_geocoding_provider(preset, api_key, client)
        .map_err(|error| error.to_string())?;
    provider.search(query).map_err(|error| error.to_string())
}

#[cfg(not(target_arch = "wasm32"))]
fn run_route(
    preset: &str,
    api_key: Option<String>,
    waypoints: &[GeoPoint],
    mode: RouteMode,
) -> Result<Vec<RouteLeg>, String> {
    if let Some(estimate_mode) = mode.estimate_mode() {
        let estimate = estimate_straight_line_route(waypoints, estimate_mode);
        return Ok(vec![RouteLeg {
            path: estimate.path,
            distance_meters: estimate.distance_meters,
            duration_seconds: estimate.duration_seconds,
        }]);
    }
    let Some(profile) = mode.routing_profile() else {
        return Ok(Vec::new());
    };
    let client = std::sync::Arc::new(terra_providers::EhttpClient);
    let preset = match preset {
        "graphhopper" => RoutingPreset::GraphHopper,
        "openrouteservice" => RoutingPreset::OpenRouteService,
        "mapbox-directions" => RoutingPreset::Mapbox,
        _ => RoutingPreset::Osrm,
    };
    let provider = terra_providers::create_routing_provider(preset, api_key, client)
        .map_err(|e| e.to_string())?;
    provider
        .route(waypoints, profile)
        .map_err(|error| error.to_string())
}

/// Measurements for the editor, formatted with the user's unit system.
pub fn formatted_measurements(
    geometry: &terra_core::PlacemarkGeometry,
    unit_system: terra_core::UnitSystem,
    coordinate_format: terra_core::CoordinateFormat,
) -> Vec<(&'static str, String)> {
    let Measurements {
        area_square_meters,
        radius_meters,
        circumference_meters,
        center,
    } = measurements(geometry);
    let mut rows = Vec::new();
    if let Some(area) = area_square_meters {
        rows.push((
            "placemarkEditor.area",
            terra_core::format_area(area, unit_system),
        ));
    }
    if let Some(radius) = radius_meters {
        rows.push((
            "placemarkEditor.radius",
            terra_core::format_distance(radius, unit_system),
        ));
    }
    if let Some(circumference) = circumference_meters {
        let key = if radius_meters.is_some() {
            "placemarkEditor.circumference"
        } else {
            "placemarkEditor.distance"
        };
        rows.push((key, terra_core::format_distance(circumference, unit_system)));
    }
    if let Some(center) = center {
        rows.push((
            "placemarkEditor.center",
            terra_core::format_coordinate(&center, coordinate_format),
        ));
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(lon: f64, lat: f64) -> GeoPoint {
        GeoPoint::new(lon, lat).expect("valid")
    }

    #[test]
    fn ruler_accumulates_segments_and_total() {
        let mut ruler = RulerState::default();
        ruler.add(point(0.0, 0.0));
        ruler.add(point(0.0, 1.0));
        ruler.add(point(0.0, 2.0));
        assert_eq!(ruler.segment_meters().len(), 2);
        assert!((ruler.total_meters() - 2.0 * ruler.segment_meters()[0]).abs() < 1.0);
        ruler.finish();
        assert!(!ruler.is_active());
        ruler.undo();
        assert_eq!(ruler.points.len(), 2);
    }

    #[test]
    fn plane_and_train_estimates_need_no_network() {
        let waypoints = vec![point(0.0, 0.0), point(0.0, 1.0)];
        let estimate = estimate_route(&waypoints, RouteMode::Plane).expect("estimate");
        assert!(estimate.duration_seconds > 0.0);
        assert!(estimate.distance_meters > 100_000.0);
        assert!(estimate_route(&waypoints, RouteMode::Car).is_none());
    }

    #[test]
    fn formatted_measurements_use_the_unit_system() {
        let geometry =
            terra_core::PlacemarkGeometry::circle(point(0.0, 0.0), 1000.0).expect("circle");
        let metric = formatted_measurements(
            &geometry,
            terra_core::UnitSystem::Metric,
            terra_core::CoordinateFormat::Decimal,
        );
        assert!(metric.iter().any(|(_, value)| value.contains("km²")));
        assert!(metric.iter().any(|(_, value)| value.contains("km")));
    }
}
