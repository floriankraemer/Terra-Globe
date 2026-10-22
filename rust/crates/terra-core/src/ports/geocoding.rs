use crate::BoxFuture;
use crate::domain::GeoPoint;

#[derive(Debug, Clone, PartialEq)]
pub struct GeocodeResult {
    pub label: String,
    pub point: GeoPoint,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProviderError {
    #[error("{0}")]
    Network(String),
    #[error("provider returned HTTP {0}")]
    Status(u16),
    #[error("unexpected provider response: {0}")]
    Malformed(String),
}

/// Geocoding port. No adapter-specific types (Nominatim shapes, keys) leak through.
pub trait GeocodingProvider: Send + Sync {
    fn search(&self, query: &str) -> BoxFuture<'_, Result<Vec<GeocodeResult>, ProviderError>>;
    /// Reverse geocode a point to a human-readable label, or `None` if none was found.
    fn reverse(&self, point: GeoPoint) -> BoxFuture<'_, Result<Option<String>, ProviderError>>;
}
