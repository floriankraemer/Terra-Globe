//! Routing providers.

pub mod graphhopper;
pub mod mapbox;
pub mod openrouteservice;
pub mod osrm;

use std::sync::Arc;

use terra_core::{DomainError, GeoPoint, PlacemarkGeometry};

use crate::ProviderError;
use crate::http::HttpClient;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoutingProfile {
    Car,
    Foot,
    Bike,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RouteLeg {
    /// Decoded route polyline. Kept as plain points so provider mapping is
    /// infallible; validation happens when the leg becomes a placemark.
    pub path: Vec<GeoPoint>,
    pub distance_meters: f64,
    pub duration_seconds: f64,
}

impl RouteLeg {
    pub fn to_geometry(&self) -> Result<PlacemarkGeometry, DomainError> {
        PlacemarkGeometry::line_string(self.path.clone(), None, None)
    }
}

/// Routing port. `[0]` is the primary route, the rest are alternatives.
pub trait RoutingProvider: Send + Sync {
    fn route(
        &self,
        waypoints: &[GeoPoint],
        profile: RoutingProfile,
    ) -> Result<Vec<RouteLeg>, ProviderError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoutingPreset {
    Osrm,
    GraphHopper,
    OpenRouteService,
    Mapbox,
}

impl RoutingPreset {
    pub fn id(self) -> &'static str {
        match self {
            Self::Osrm => "osrm",
            Self::GraphHopper => "graphhopper",
            Self::OpenRouteService => "openrouteservice",
            Self::Mapbox => "mapbox-directions",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Osrm => "OSRM",
            Self::GraphHopper => "GraphHopper",
            Self::OpenRouteService => "OpenRouteService",
            Self::Mapbox => "Mapbox Directions",
        }
    }

    pub fn requires_api_key(self) -> bool {
        !matches!(self, Self::Osrm)
    }
}

pub fn create_routing_provider(
    preset: RoutingPreset,
    api_key: Option<String>,
    client: Arc<dyn HttpClient>,
) -> Result<Box<dyn RoutingProvider>, ProviderError> {
    Ok(match preset {
        RoutingPreset::Osrm => Box::new(osrm::OsrmRoutingProvider::new(client)),
        RoutingPreset::GraphHopper => Box::new(graphhopper::GraphHopperRoutingProvider::new(
            require_key(preset, api_key)?,
            client,
        )),
        RoutingPreset::OpenRouteService => {
            Box::new(openrouteservice::OpenRouteServiceRoutingProvider::new(
                require_key(preset, api_key)?,
                client,
            ))
        }
        RoutingPreset::Mapbox => Box::new(mapbox::MapboxRoutingProvider::new(
            require_key(preset, api_key)?,
            client,
        )),
    })
}

fn require_key(preset: RoutingPreset, api_key: Option<String>) -> Result<String, ProviderError> {
    let key = api_key.unwrap_or_default();
    if key.trim().is_empty() {
        return Err(ProviderError::MissingApiKey(preset.id().to_string()));
    }
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http::FakeHttpClient;

    #[test]
    fn key_backed_presets_require_a_key() {
        for preset in [
            RoutingPreset::GraphHopper,
            RoutingPreset::OpenRouteService,
            RoutingPreset::Mapbox,
        ] {
            let error = create_routing_provider(preset, None, Arc::new(FakeHttpClient::default()))
                .err()
                .expect("must require a key");
            assert!(matches!(error, ProviderError::MissingApiKey(_)));
        }
        assert!(
            create_routing_provider(
                RoutingPreset::Osrm,
                None,
                Arc::new(FakeHttpClient::default())
            )
            .is_ok()
        );
    }

    #[test]
    fn preset_ids_match_the_web_app() {
        assert_eq!(RoutingPreset::Osrm.id(), "osrm");
        assert_eq!(RoutingPreset::OpenRouteService.id(), "openrouteservice");
        assert_eq!(RoutingPreset::Mapbox.id(), "mapbox-directions");
        assert!(!RoutingPreset::Osrm.requires_api_key());
    }
}
