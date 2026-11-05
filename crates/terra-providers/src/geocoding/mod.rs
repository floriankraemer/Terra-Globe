//! Geocoding providers: forward search and reverse geocoding.

pub mod locationiq;
pub mod mapbox;
pub mod nominatim;
pub mod opencage;

use std::sync::Arc;

use terra_core::GeoPoint;

use crate::ProviderError;
use crate::http::HttpClient;

#[derive(Debug, Clone, PartialEq)]
pub struct GeocodeResult {
    pub label: String,
    pub point: GeoPoint,
}

/// Geocoding port. No provider response shape or API key leaks through here.
pub trait GeocodingProvider: Send + Sync {
    fn search(&self, query: &str) -> Result<Vec<GeocodeResult>, ProviderError>;
    /// Human-readable label for a point, or `None` when nothing was found.
    fn reverse(&self, point: GeoPoint) -> Result<Option<String>, ProviderError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeocodingPreset {
    Nominatim,
    LocationIq,
    Mapbox,
    OpenCage,
}

impl GeocodingPreset {
    pub fn id(self) -> &'static str {
        match self {
            Self::Nominatim => "nominatim",
            Self::LocationIq => "locationiq",
            Self::Mapbox => "mapbox-geocoding",
            Self::OpenCage => "opencage",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Nominatim => "OpenStreetMap Nominatim",
            Self::LocationIq => "LocationIQ",
            Self::Mapbox => "Mapbox Geocoding",
            Self::OpenCage => "OpenCage",
        }
    }

    pub fn requires_api_key(self) -> bool {
        !matches!(self, Self::Nominatim)
    }
}

pub fn create_geocoding_provider(
    preset: GeocodingPreset,
    api_key: Option<String>,
    client: Arc<dyn HttpClient>,
) -> Result<Box<dyn GeocodingProvider>, ProviderError> {
    Ok(match preset {
        GeocodingPreset::Nominatim => Box::new(nominatim::NominatimGeocodingProvider::new(client)),
        GeocodingPreset::LocationIq => Box::new(locationiq::LocationIqGeocodingProvider::new(
            require_key(preset, api_key)?,
            client,
        )),
        GeocodingPreset::Mapbox => Box::new(mapbox::MapboxGeocodingProvider::new(
            require_key(preset, api_key)?,
            client,
        )),
        GeocodingPreset::OpenCage => Box::new(opencage::OpenCageGeocodingProvider::new(
            require_key(preset, api_key)?,
            client,
        )),
    })
}

fn require_key(preset: GeocodingPreset, api_key: Option<String>) -> Result<String, ProviderError> {
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
            GeocodingPreset::LocationIq,
            GeocodingPreset::Mapbox,
            GeocodingPreset::OpenCage,
        ] {
            let error =
                create_geocoding_provider(preset, None, Arc::new(FakeHttpClient::default()))
                    .err()
                    .expect("must require a key");
            assert!(matches!(error, ProviderError::MissingApiKey(_)));
        }
        let nominatim = create_geocoding_provider(
            GeocodingPreset::Nominatim,
            None,
            Arc::new(FakeHttpClient::default()),
        );
        assert!(nominatim.is_ok());
    }

    #[test]
    fn preset_ids_match_the_web_app() {
        assert_eq!(GeocodingPreset::LocationIq.id(), "locationiq");
        assert_eq!(GeocodingPreset::Mapbox.id(), "mapbox-geocoding");
        assert_eq!(GeocodingPreset::OpenCage.id(), "opencage");
        assert!(!GeocodingPreset::Nominatim.requires_api_key());
        assert!(GeocodingPreset::Mapbox.requires_api_key());
    }
}
