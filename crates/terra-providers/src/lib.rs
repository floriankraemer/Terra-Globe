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

    pub fn esri_world_imagery() -> Self {
        Self {
            id: "esri-world-imagery".to_string(),
            name: "Satellite (Esri World Imagery)".to_string(),
            url: "https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{z}/{y}/{x}".to_string(),
            subdomains: Vec::new(),
            credit: "Esri, Maxar, Earthstar Geographics, and the GIS User Community".to_string(),
            maximum_level: 19,
        }
    }

    /// JPEG tiles; the `image` crate's `jpeg` feature decodes them alongside PNG.
    pub fn s2cloudless() -> Self {
        Self {
            id: "s2cloudless".to_string(),
            name: "Satellite (Sentinel-2 cloudless)".to_string(),
            url: "https://tiles.maps.eox.at/wmts/1.0.0/s2cloudless-2020_3857/default/g/{z}/{y}/{x}.jpg".to_string(),
            subdomains: Vec::new(),
            credit: "Sentinel-2 cloudless – https://s2maps.eu by EOX IT Services GmbH (contains modified Copernicus Sentinel data 2020)".to_string(),
            maximum_level: 15,
        }
    }

    /// Terrarium-encoded elevation tiles (AWS Terrain Tiles): no key, free,
    /// Web-Mercator XYZ, 256x256 PNG. Height in metres is
    /// `R * 256 + G + B / 256 - 32768`. Not a drapeable image, so it is not
    /// listed among the base maps or overlays. The server sends no CORS
    /// headers, so only the desktop build can use it.
    pub fn terrain_terrarium() -> Self {
        Self {
            id: "terrain-terrarium".to_string(),
            name: "Terrain (Terrarium)".to_string(),
            url: "https://s3.amazonaws.com/elevation-tiles-prod/terrarium/{z}/{x}/{y}.png"
                .to_string(),
            subdomains: Vec::new(),
            credit: "Terrain: Mapzen Terrain Tiles on AWS (SRTM, GMTED2010, ETOPO1 and others)"
                .to_string(),
            maximum_level: 15,
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
    vec![
        TileSource::open_street_map(),
        TileSource::open_topo_map(),
        TileSource::esri_world_imagery(),
        TileSource::s2cloudless(),
    ]
}

/// Transparent overlays drawn above a base map. Base maps are deliberately absent.
pub fn overlay_presets() -> Vec<TileSource> {
    vec![
        TileSource {
            id: "openrailwaymap".to_string(),
            name: "OpenRailwayMap".to_string(),
            url: "https://{s}.tiles.openrailwaymap.org/standard/{z}/{x}/{y}.png".to_string(),
            subdomains: vec!["a".to_string(), "b".to_string(), "c".to_string()],
            credit: "© OpenStreetMap contributors | © OpenRailwayMap (CC-BY-SA)".to_string(),
            maximum_level: 19,
        },
        TileSource {
            id: "openseamap".to_string(),
            name: "OpenSeaMap".to_string(),
            url: "https://tiles.openseamap.org/seamark/{z}/{x}/{y}.png".to_string(),
            subdomains: Vec::new(),
            credit: "© OpenStreetMap contributors | © OpenSeaMap (CC-BY-SA)".to_string(),
            maximum_level: 18,
        },
        TileSource {
            id: "waymarked-hiking".to_string(),
            name: "Waymarked Trails: Hiking".to_string(),
            url: "https://tile.waymarkedtrails.org/hiking/{z}/{x}/{y}.png".to_string(),
            subdomains: Vec::new(),
            credit: "© OpenStreetMap contributors | © Waymarked Trails (CC-BY-SA)".to_string(),
            maximum_level: 18,
        },
        TileSource {
            id: "waymarked-cycling".to_string(),
            name: "Waymarked Trails: Cycling".to_string(),
            url: "https://tile.waymarkedtrails.org/cycling/{z}/{x}/{y}.png".to_string(),
            subdomains: Vec::new(),
            credit: "© OpenStreetMap contributors | © Waymarked Trails (CC-BY-SA)".to_string(),
            maximum_level: 18,
        },
        TileSource {
            id: "esri-labels".to_string(),
            name: "Places & boundaries (Esri)".to_string(),
            url: "https://server.arcgisonline.com/ArcGIS/rest/services/Reference/World_Boundaries_and_Places/MapServer/tile/{z}/{y}/{x}".to_string(),
            subdomains: Vec::new(),
            credit: "Esri".to_string(),
            maximum_level: 19,
        },
    ]
}

/// Looks up a transparent overlay by its stable id.
pub fn overlay_preset(id: &str) -> Option<TileSource> {
    overlay_presets().into_iter().find(|source| source.id == id)
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

    #[test]
    fn overlay_presets_have_unique_ids() {
        let presets = overlay_presets();
        let mut ids: Vec<&str> = presets.iter().map(|source| source.id.as_str()).collect();
        ids.sort_unstable();
        let count = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), count);
        assert!(count >= 4);
    }

    #[test]
    fn overlay_presets_are_valid_tile_templates_with_credit() {
        for source in overlay_presets() {
            source.validate().expect("overlay preset must be valid");
            assert!(!source.credit.trim().is_empty(), "{}", source.id);
        }
    }

    #[test]
    fn overlay_preset_finds_each_overlay_by_id() {
        for source in overlay_presets() {
            let found = overlay_preset(&source.id).expect("preset must be found by its id");
            assert_eq!(found, source);
        }
        assert!(overlay_preset("openrailwaymap").is_some());
        assert!(overlay_preset("openseamap").is_some());
        assert!(overlay_preset("waymarked-hiking").is_some());
        assert!(overlay_preset("waymarked-cycling").is_some());
    }

    #[test]
    fn base_maps_and_unknown_ids_are_not_overlays() {
        assert!(overlay_preset("osm").is_none());
        assert!(overlay_preset("opentopomap").is_none());
        assert!(overlay_preset("nope").is_none());
        assert!(overlay_preset("").is_none());
    }

    #[test]
    fn terrain_source_is_terrarium_on_aws_with_credit() {
        let source = TileSource::terrain_terrarium();
        source.validate().expect("valid template");
        assert_eq!(source.maximum_level, 15);
        assert_eq!(
            source.tile_url(3, 4, 2, None),
            "https://s3.amazonaws.com/elevation-tiles-prod/terrarium/3/4/2.png"
        );
        assert!(source.credit.contains("Mapzen Terrain Tiles"));
        assert!(
            !builtin_tile_sources().iter().any(|s| s.id == source.id)
                && !overlay_presets().iter().any(|s| s.id == source.id),
            "elevation data is not an imagery layer"
        );
    }

    #[test]
    fn satellite_base_maps_are_builtin() {
        let sources = builtin_tile_sources();
        assert!(
            sources
                .iter()
                .any(|source| source.id == "esri-world-imagery")
        );
        assert!(sources.iter().any(|source| source.id == "s2cloudless"));
    }

    #[test]
    fn source_ids_are_unique_across_base_maps_and_overlays() {
        let mut ids: Vec<String> = builtin_tile_sources()
            .into_iter()
            .chain(overlay_presets())
            .map(|source| source.id)
            .collect();
        let count = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), count);
    }

    #[test]
    fn esri_world_imagery_puts_y_before_x() {
        let source = builtin_tile_sources()
            .into_iter()
            .find(|source| source.id == "esri-world-imagery")
            .expect("esri imagery is builtin");
        assert_eq!(
            source.tile_url(5, 17, 11, None),
            "https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/5/11/17"
        );
        assert_eq!(source.maximum_level, 19);
    }

    #[test]
    fn s2cloudless_serves_jpeg_tiles_with_y_before_x() {
        let source = builtin_tile_sources()
            .into_iter()
            .find(|source| source.id == "s2cloudless")
            .expect("s2cloudless is builtin");
        assert!(source.tile_url(5, 17, 11, None).ends_with("/5/11/17.jpg"));
        assert_eq!(source.maximum_level, 15);
    }

    #[test]
    fn esri_labels_overlay_is_registered() {
        let source = overlay_preset("esri-labels").expect("esri labels overlay exists");
        assert_eq!(
            source.tile_url(5, 17, 11, None),
            "https://server.arcgisonline.com/ArcGIS/rest/services/Reference/World_Boundaries_and_Places/MapServer/tile/5/11/17"
        );
    }

    #[test]
    fn satellite_base_maps_are_not_overlays() {
        assert!(overlay_preset("esri-world-imagery").is_none());
        assert!(overlay_preset("s2cloudless").is_none());
    }

    #[test]
    fn unknown_builtin_id_is_not_found() {
        let found = builtin_tile_sources()
            .into_iter()
            .find(|source| source.id == "nope");
        assert!(found.is_none());
    }
}
