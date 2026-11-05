//! Tile, geocoding and routing provider definitions.
//!
//! The app ships no proprietary data: built-in layers are open ones, and every
//! other source is a preset the user enables with their own key (BYOK).

use serde::{Deserialize, Serialize};
use terra_core::DomainError;

pub mod geocoding;
pub mod http;
pub mod routing;
pub mod secrets;

pub use geocoding::{GeocodeResult, GeocodingPreset, GeocodingProvider, create_geocoding_provider};
pub use http::{EhttpClient, FakeHttpClient, HttpClient, HttpError, HttpRequest, HttpResponse};
pub use routing::{
    RouteLeg, RoutingPreset, RoutingProfile, RoutingProvider, create_routing_provider,
};
pub use secrets::{InMemorySecretStore, SecretError, SecretStore};

/// A raster tile source the globe can drape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TileSource {
    pub id: String,
    pub name: String,
    /// URL template with `{z}`, `{x}`, `{y}`, optional `{s}` and `{api_key}`.
    pub url: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub subdomains: Vec<String>,
    pub credit: String,
    pub maximum_level: u8,
}

impl TileSource {
    pub fn open_street_map() -> Self {
        Self {
            id: "osm".to_string(),
            name: "OpenStreetMap".to_string(),
            url: "https://tile.openstreetmap.org/{z}/{x}/{y}.png".to_string(),
            subdomains: Vec::new(),
            credit: "© OpenStreetMap contributors".to_string(),
            maximum_level: 19,
        }
    }

    pub fn open_topo_map() -> Self {
        Self {
            id: "opentopomap".to_string(),
            name: "OpenTopoMap".to_string(),
            url: "https://{s}.tile.opentopomap.org/{z}/{x}/{y}.png".to_string(),
            subdomains: vec!["a".to_string(), "b".to_string(), "c".to_string()],
            credit: "© OpenStreetMap contributors, SRTM | © OpenTopoMap (CC-BY-SA)".to_string(),
            maximum_level: 17,
        }
    }

    /// Expands a tile URL for one tile, rotating subdomains deterministically so
    /// the same tile always maps to the same host (better HTTP caching).
    pub fn tile_url(&self, z: u8, x: u32, y: u32, api_key: Option<&str>) -> String {
        let mut url = self.url.replace("{z}", &z.to_string());
        url = url.replace("{x}", &x.to_string());
        url = url.replace("{y}", &y.to_string());
        if url.contains("{s}") && !self.subdomains.is_empty() {
            let index =
                (u64::from(x) + 2 * u64::from(y) + u64::from(z)) as usize % self.subdomains.len();
            let subdomain = self
                .subdomains
                .get(index)
                .map(String::as_str)
                .unwrap_or_default();
            url = url.replace("{s}", subdomain);
        }
        url = url.replace("{api_key}", api_key.unwrap_or_default());
        url
    }

    pub fn validate(&self) -> Result<(), DomainError> {
        if self.name.trim().is_empty() {
            return Err(DomainError::EmptyName);
        }
        if !self.url.contains("{z}") || !self.url.contains("{x}") || !self.url.contains("{y}") {
            return Err(DomainError::EmptyImageUrl);
        }
        Ok(())
    }
}

/// Built-in sources, in menu order. Mirrors `BUILTIN_TILE_SOURCES` in the web app.
pub fn builtin_tile_sources() -> Vec<TileSource> {
    vec![TileSource::open_street_map(), TileSource::open_topo_map()]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProviderKind {
    Tile,
    Geocoding,
    Routing,
}

/// A user-configured provider. `preset` names a catalog entry; `enabled`
/// reflects the UI toggle, not whether a key is present.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConfig {
    pub id: String,
    pub kind: ProviderKind,
    pub preset: String,
    pub name: String,
    pub enabled: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("missing API key for provider {0}")]
    MissingApiKey(String),
    #[error(transparent)]
    Http(#[from] crate::http::HttpError),
    #[error("request failed: {0}")]
    Request(String),
    #[error("failed to decode provider response: {0}")]
    Decode(String),
    #[error("provider {0} is not implemented yet")]
    NotImplemented(&'static str),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tile_url_substitutes_coordinates() {
        let osm = TileSource::open_street_map();
        assert_eq!(
            osm.tile_url(3, 4, 2, None),
            "https://tile.openstreetmap.org/3/4/2.png"
        );
    }

    #[test]
    fn tile_url_rotates_subdomains_deterministically() {
        let topo = TileSource::open_topo_map();
        let first = topo.tile_url(5, 8, 9, None);
        let second = topo.tile_url(5, 8, 9, None);
        assert_eq!(first, second);
        assert!(
            first.starts_with("https://a.")
                || first.starts_with("https://b.")
                || first.starts_with("https://c.")
        );
    }

    #[test]
    fn api_key_placeholder_is_substituted() {
        let mut source = TileSource::open_street_map();
        source.url = "https://example.test/{z}/{x}/{y}?key={api_key}".to_string();
        assert_eq!(
            source.tile_url(1, 1, 1, Some("secret")),
            "https://example.test/1/1/1?key=secret"
        );
    }

    #[test]
    fn builtin_sources_are_valid() {
        for source in builtin_tile_sources() {
            source.validate().expect("builtin source must be valid");
        }
    }
}
