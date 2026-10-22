use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TilePresetId {
    MapboxStreets,
    MaptilerStreets,
    ThunderforestCycle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GeocodingPresetId {
    Locationiq,
    MapboxGeocoding,
    Opencage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RoutingPresetId {
    Osrm,
    Graphhopper,
    Openrouteservice,
    MapboxDirections,
}

/// A user-configured external data provider ("bring your own key").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ProviderConfig {
    Tile {
        id: String,
        preset: TilePresetId,
        name: String,
        enabled: bool,
    },
    Geocoding {
        id: String,
        preset: GeocodingPresetId,
        name: String,
        enabled: bool,
    },
    Routing {
        id: String,
        preset: RoutingPresetId,
        name: String,
        enabled: bool,
    },
}

impl ProviderConfig {
    pub fn id(&self) -> &str {
        match self {
            Self::Tile { id, .. } | Self::Geocoding { id, .. } | Self::Routing { id, .. } => id,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Self::Tile { name, .. } | Self::Geocoding { name, .. } | Self::Routing { name, .. } => {
                name
            }
        }
    }

    pub fn enabled(&self) -> bool {
        match self {
            Self::Tile { enabled, .. }
            | Self::Geocoding { enabled, .. }
            | Self::Routing { enabled, .. } => *enabled,
        }
    }

    pub fn set_enabled(&mut self, value: bool) {
        match self {
            Self::Tile { enabled, .. }
            | Self::Geocoding { enabled, .. }
            | Self::Routing { enabled, .. } => *enabled = value,
        }
    }

    pub fn kind(&self) -> ProviderKind {
        match self {
            Self::Tile { .. } => ProviderKind::Tile,
            Self::Geocoding { .. } => ProviderKind::Geocoding,
            Self::Routing { .. } => ProviderKind::Routing,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProviderKind {
    Tile,
    Geocoding,
    Routing,
}
