use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AnchorUnits {
    Fraction,
    Pixels,
    InsetPixels,
}

/// Fraction (0-1) or pixel offset from an edge, per KML's vec2.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenOverlayAnchor {
    pub x: f64,
    pub y: f64,
    pub x_units: AnchorUnits,
    pub y_units: AnchorUnits,
}

/// Screen-space image pinned to the viewport (KML ScreenOverlay).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenOverlay {
    pub id: String,
    pub folder_id: Option<String>,
    pub name: String,
    pub image_url: String,
    #[serde(rename = "overlayXY")]
    pub overlay_xy: ScreenOverlayAnchor,
    #[serde(rename = "screenXY")]
    pub screen_xy: ScreenOverlayAnchor,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_x: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_y: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rotation: Option<f64>,
    pub visibility: bool,
    pub order: f64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NewScreenOverlay {
    pub name: String,
    pub folder_id: Option<String>,
    pub image_url: String,
    pub overlay_xy: ScreenOverlayAnchor,
    pub screen_xy: ScreenOverlayAnchor,
    pub size_x: Option<f64>,
    pub size_y: Option<f64>,
    pub rotation: Option<f64>,
    pub visibility: Option<bool>,
    pub order: Option<f64>,
}
