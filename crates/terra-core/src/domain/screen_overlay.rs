//! KML ScreenOverlay entity, mirroring `packages/core/src/domain/screenOverlay.ts`.

use serde::{Deserialize, Serialize};

use super::DomainError;
use super::ids::new_id;
use super::time::now_iso8601;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AnchorUnits {
    Fraction,
    Pixels,
    InsetPixels,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenOverlayAnchor {
    pub x: f64,
    pub y: f64,
    pub x_units: AnchorUnits,
    pub y_units: AnchorUnits,
}

impl Default for ScreenOverlayAnchor {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            x_units: AnchorUnits::Fraction,
            y_units: AnchorUnits::Fraction,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenOverlay {
    pub id: String,
    pub folder_id: Option<String>,
    pub name: String,
    pub image_url: String,
    // The web app's JSON uses these exact keys; `camelCase` would produce
    // `overlayXy`, which would not match rows already on disk.
    #[serde(rename = "overlayXY", alias = "overlayXy")]
    pub overlay_xy: ScreenOverlayAnchor,
    #[serde(rename = "screenXY", alias = "screenXy")]
    pub screen_xy: ScreenOverlayAnchor,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_x: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_y: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rotation: Option<f64>,
    pub visibility: bool,
    pub order: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Default)]
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
    pub order: Option<i64>,
}

pub fn create_screen_overlay(input: NewScreenOverlay) -> Result<ScreenOverlay, DomainError> {
    if input.name.trim().is_empty() {
        return Err(DomainError::EmptyName);
    }
    if input.image_url.trim().is_empty() {
        return Err(DomainError::EmptyImageUrl);
    }
    let now = now_iso8601();
    Ok(ScreenOverlay {
        id: new_id(),
        folder_id: input.folder_id,
        name: input.name,
        image_url: input.image_url,
        overlay_xy: input.overlay_xy,
        screen_xy: input.screen_xy,
        size_x: input.size_x,
        size_y: input.size_y,
        rotation: input.rotation,
        visibility: input.visibility.unwrap_or(true),
        order: input.order.unwrap_or(0),
        created_at: now.clone(),
        updated_at: now,
    })
}
