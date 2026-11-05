//! Style entity, mirroring `packages/core/src/domain/style.ts`.

use serde::{Deserialize, Serialize};

use super::DomainError;
use super::ids::new_id;

pub const MARKER_ICON_IDS: [&str; 6] = ["map-pin", "pin", "flag", "star", "landmark", "navigation"];

pub const DEFAULT_MARKER_ICON: MarkerIconId = MarkerIconId::MapPin;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum MarkerIconId {
    #[default]
    #[serde(rename = "map-pin")]
    MapPin,
    #[serde(rename = "pin")]
    Pin,
    #[serde(rename = "flag")]
    Flag,
    #[serde(rename = "star")]
    Star,
    #[serde(rename = "landmark")]
    Landmark,
    #[serde(rename = "navigation")]
    Navigation,
}

impl MarkerIconId {
    pub fn all() -> [Self; 6] {
        [
            Self::MapPin,
            Self::Pin,
            Self::Flag,
            Self::Star,
            Self::Landmark,
            Self::Navigation,
        ]
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::all().into_iter().find(|icon| icon.as_str() == id)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::MapPin => "map-pin",
            Self::Pin => "pin",
            Self::Flag => "flag",
            Self::Star => "star",
            Self::Landmark => "landmark",
            Self::Navigation => "navigation",
        }
    }
}

/// The "highlight" half of a KML StyleMap. Rendering always uses the normal
/// fields; this exists so StyleMap round-trips without losing data.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StyleHighlight {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outline_color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outline_width: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill_color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill_opacity: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_scale: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label_color: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Style {
    pub id: String,
    pub outline_enabled: bool,
    pub outline_color: String,
    pub outline_width: f64,
    pub filled: bool,
    pub fill_color: String,
    pub fill_opacity: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon_scale: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub marker_icon: Option<MarkerIconId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label_color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub highlight: Option<StyleHighlight>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub balloon_text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub balloon_bg_color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub balloon_text_color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub balloon_display_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub list_item_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item_icon_href: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewStyle {
    pub outline_color: String,
    pub outline_width: f64,
    pub fill_color: String,
    pub fill_opacity: f64,
    pub outline_enabled: Option<bool>,
    pub filled: Option<bool>,
    pub icon_url: Option<String>,
    pub icon_scale: Option<f64>,
    pub marker_icon: Option<MarkerIconId>,
    pub label_color: Option<String>,
}

pub fn create_style(input: NewStyle) -> Result<Style, DomainError> {
    if !input.outline_width.is_finite() || input.outline_width < 0.0 {
        return Err(DomainError::InvalidOutlineWidth(input.outline_width));
    }
    if !input.fill_opacity.is_finite() || !(0.0..=1.0).contains(&input.fill_opacity) {
        return Err(DomainError::InvalidFillOpacity(input.fill_opacity));
    }
    Ok(Style {
        id: new_id(),
        outline_enabled: input.outline_enabled.unwrap_or(true),
        outline_color: input.outline_color,
        outline_width: input.outline_width,
        filled: input.filled.unwrap_or(false),
        fill_color: input.fill_color,
        fill_opacity: input.fill_opacity,
        icon_url: input.icon_url,
        icon_scale: input.icon_scale,
        marker_icon: input.marker_icon,
        label_color: input.label_color,
        highlight: None,
        balloon_text: None,
        balloon_bg_color: None,
        balloon_text_color: None,
        balloon_display_mode: None,
        list_item_type: None,
        item_icon_href: None,
    })
}

impl Default for Style {
    fn default() -> Self {
        Self {
            id: String::new(),
            outline_enabled: true,
            outline_color: "#3388ff".to_string(),
            outline_width: 2.0,
            filled: false,
            fill_color: "#3388ff".to_string(),
            fill_opacity: 0.4,
            icon_url: None,
            icon_scale: None,
            marker_icon: None,
            label_color: None,
            highlight: None,
            balloon_text: None,
            balloon_bg_color: None,
            balloon_text_color: None,
            balloon_display_mode: None,
            list_item_type: None,
            item_icon_href: None,
        }
    }
}
