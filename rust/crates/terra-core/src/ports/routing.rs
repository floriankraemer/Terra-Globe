use crate::BoxFuture;
use crate::domain::{GeoPoint, PlacemarkGeometry};
use crate::ports::geocoding::ProviderError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RoutingProfile {
    Car,
    Foot,
    Bike,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RouteLeg {
    /// Always a `PlacemarkGeometry::LineString`.
    pub geometry: PlacemarkGeometry,
    pub distance_meters: f64,
    pub duration_seconds: f64,
}

/// Routing port. `[0]` is the primary route, the rest are alternatives.
pub trait RoutingProvider: Send + Sync {
    fn route(
        &self,
        waypoints: &[GeoPoint],
        profile: RoutingProfile,
    ) -> BoxFuture<'_, Result<Vec<RouteLeg>, ProviderError>>;
}
