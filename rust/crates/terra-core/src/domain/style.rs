use serde::{Deserialize, Serialize};

/// Built-in marker icon choices for Point placemarks (Lucide icons).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MarkerIconId {
    MapPin,
    Pin,
    Flag,
    Star,
    Landmark,
    Navigation,
}

impl MarkerIconId {
    pub const ALL: [Self; 6] = [
        Self::MapPin,
        Self::Pin,
        Self::Flag,
        Self::Star,
        Self::Landmark,
        Self::Navigation,
    ];
    pub const DEFAULT: Self = Self::MapPin;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BalloonDisplayMode {
    Default,
    Hide,
}

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
    /// Built-in marker icon for Point placemarks. Ignored when `icon_url` is set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub marker_icon: Option<MarkerIconId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label_color: Option<String>,
    /// From a KML StyleMap's "highlight" pair. Rendering always uses the normal fields.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub highlight: Option<StyleHighlight>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub balloon_text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub balloon_bg_color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub balloon_text_color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub balloon_display_mode: Option<BalloonDisplayMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub list_item_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item_icon_href: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
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
    pub highlight: Option<StyleHighlight>,
    pub balloon_text: Option<String>,
    pub balloon_bg_color: Option<String>,
    pub balloon_text_color: Option<String>,
    pub balloon_display_mode: Option<BalloonDisplayMode>,
    pub list_item_type: Option<String>,
    pub item_icon_href: Option<String>,
}
