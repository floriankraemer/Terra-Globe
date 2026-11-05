//! User settings shared by the desktop and browser shells.
//!
//! These used to live in the webview's `localStorage`; the Rust app persists the
//! same model to a JSON file in the platform config directory.

use serde::{Deserialize, Serialize};

use crate::units::{CoordinateFormat, UnitSystem};

pub const DEFAULT_LANGUAGE: &str = "en";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub unit_system: UnitSystem,
    pub coordinate_format: CoordinateFormat,
    pub language: String,
    pub auto_save: bool,
    /// Selected preset ids; keys live in the secret store, never here.
    pub geocoding_preset: String,
    pub routing_preset: String,
    pub tile_source_id: String,
    pub show_labels: bool,
    /// "3d", "2d" or "columbus".
    pub scene_mode: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            unit_system: UnitSystem::Metric,
            coordinate_format: CoordinateFormat::Decimal,
            language: DEFAULT_LANGUAGE.to_string(),
            auto_save: true,
            geocoding_preset: "nominatim".to_string(),
            routing_preset: "osrm".to_string(),
            tile_source_id: "osm".to_string(),
            show_labels: true,
            scene_mode: "3d".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_web_app() {
        let settings = Settings::default();
        assert_eq!(settings.unit_system, UnitSystem::Metric);
        assert_eq!(settings.coordinate_format, CoordinateFormat::Decimal);
        assert_eq!(settings.language, DEFAULT_LANGUAGE);
        assert!(settings.auto_save);
        assert_eq!(settings.geocoding_preset, "nominatim");
        assert_eq!(settings.routing_preset, "osrm");
        assert_eq!(settings.tile_source_id, "osm");
        assert!(settings.show_labels);
        assert_eq!(settings.scene_mode, "3d");
    }

    #[test]
    fn missing_fields_fall_back_to_defaults() {
        // A settings file written by an older build must not fail to load.
        let settings: Settings = serde_json::from_str(r#"{"language":"de"}"#).expect("parses");
        assert_eq!(settings.language, "de");
        assert_eq!(settings.unit_system, UnitSystem::Metric);
        assert!(settings.auto_save);
    }

    #[test]
    fn serializes_with_camel_case_keys() {
        let value = serde_json::to_value(Settings::default()).expect("serializable");
        assert!(value.get("unitSystem").is_some());
        assert!(value.get("coordinateFormat").is_some());
        assert!(value.get("autoSave").is_some());
    }
}
