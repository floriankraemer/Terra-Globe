//! User settings shared by the desktop and browser shells.
//!
//! These used to live in the webview's `localStorage`; the Rust app persists the
//! same model to a JSON file in the platform config directory.

use std::collections::HashSet;

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
    /// Transparent overlay layers in stacking order. Ids are resolved against the
    /// app's overlay catalog; unknown ids are ignored there, not here.
    pub overlay_layers: Vec<OverlayLayerSetting>,
    /// Displace the globe by elevation data (3D mode, desktop only: the
    /// elevation source cannot be reached from a browser). Settings written
    /// before terrain existed load with it on.
    pub terrain_enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayLayerSetting {
    pub id: String,
    pub enabled: bool,
    pub opacity: f32,
}

impl Settings {
    /// Clamps overlay opacity into `0.0..=1.0` (NaN becomes fully opaque) and drops
    /// repeated overlay ids, keeping the first occurrence.
    pub fn normalized(mut self) -> Self {
        for layer in &mut self.overlay_layers {
            layer.opacity = if layer.opacity.is_nan() {
                1.0
            } else {
                layer.opacity.clamp(0.0, 1.0)
            };
        }
        let mut seen = HashSet::new();
        self.overlay_layers
            .retain(|layer| seen.insert(layer.id.clone()));
        self
    }
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
            overlay_layers: Vec::new(),
            terrain_enabled: true,
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

    fn layer(id: &str, enabled: bool, opacity: f32) -> OverlayLayerSetting {
        OverlayLayerSetting {
            id: id.to_string(),
            enabled,
            opacity,
        }
    }

    fn with_layers(layers: Vec<OverlayLayerSetting>) -> Settings {
        Settings {
            overlay_layers: layers,
            ..Settings::default()
        }
    }

    #[test]
    fn overlay_layers_round_trip_through_json() {
        let settings = with_layers(vec![layer("openrailwaymap", true, 0.6)]);
        let text = serde_json::to_string(&settings).expect("serializable");
        assert!(
            text.contains(
                r#""overlayLayers":[{"id":"openrailwaymap","enabled":true,"opacity":0.6}]"#
            )
        );
        let parsed: Settings = serde_json::from_str(&text).expect("parses");
        assert_eq!(parsed, settings);
    }

    #[test]
    fn json_without_overlay_layers_loads_with_no_overlays() {
        let settings: Settings =
            serde_json::from_str(r#"{"unitSystem":"metric","language":"en"}"#).expect("parses");
        assert!(settings.overlay_layers.is_empty());
    }

    #[test]
    fn terrain_is_on_by_default_and_old_settings_load_with_it_on() {
        assert!(Settings::default().terrain_enabled);
        let old: Settings =
            serde_json::from_str(r#"{"unitSystem":"metric","language":"de"}"#).expect("parses");
        assert!(old.terrain_enabled);
        assert_eq!(old.language, "de");
    }

    #[test]
    fn terrain_choice_round_trips_and_uses_a_camel_case_key() {
        let off = Settings {
            terrain_enabled: false,
            ..Settings::default()
        };
        let text = serde_json::to_string(&off).expect("serializable");
        assert!(text.contains(r#""terrainEnabled":false"#), "{text}");
        let parsed: Settings = serde_json::from_str(&text).expect("parses");
        assert!(!parsed.terrain_enabled);
        assert!(serde_json::from_str::<Settings>(r#"{"terrainEnabled":"yes"}"#).is_err());
    }

    #[test]
    fn default_has_no_overlay_layers() {
        assert!(Settings::default().overlay_layers.is_empty());
    }

    #[test]
    fn malformed_overlay_entries_fail_to_parse() {
        let missing_field = serde_json::from_str::<Settings>(
            r#"{"overlayLayers":[{"id":"openseamap","enabled":true}]}"#,
        );
        assert!(missing_field.is_err());
        let wrong_type = serde_json::from_str::<Settings>(
            r#"{"overlayLayers":[{"id":5,"enabled":true,"opacity":0.5}]}"#,
        );
        assert!(wrong_type.is_err());
    }

    #[test]
    fn normalized_clamps_opacity_into_unit_range() {
        let settings = with_layers(vec![
            layer("high", true, 2.5),
            layer("low", true, -1.0),
            layer("nan", true, f32::NAN),
            layer("kept", false, 0.4),
        ])
        .normalized();
        let opacities: Vec<f32> = settings.overlay_layers.iter().map(|l| l.opacity).collect();
        assert_eq!(opacities, vec![1.0, 0.0, 1.0, 0.4]);
    }

    #[test]
    fn normalized_drops_duplicate_ids_keeping_first() {
        let settings = with_layers(vec![
            layer("a", true, 0.2),
            layer("a", false, 0.9),
            layer("b", true, 1.0),
        ])
        .normalized();
        assert_eq!(
            settings.overlay_layers,
            vec![layer("a", true, 0.2), layer("b", true, 1.0)]
        );
    }

    #[test]
    fn serializes_with_camel_case_keys() {
        let value = serde_json::to_value(Settings::default()).expect("serializable");
        assert!(value.get("unitSystem").is_some());
        assert!(value.get("coordinateFormat").is_some());
        assert!(value.get("autoSave").is_some());
    }
}
