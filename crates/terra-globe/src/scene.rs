//! What the application asks the globe to draw, beyond the imagery tiles.
//!
//! Kept independent of egui so the renderer stays UI-agnostic and the scene can
//! be unit-tested.

use terra_core::{GeoPoint, PlacemarkGeometry, Style};

#[derive(Debug, Clone, PartialEq)]
pub struct SceneEntry {
    pub id: String,
    pub geometry: PlacemarkGeometry,
    pub style: Style,
    pub visible: bool,
    pub selected: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Scene {
    pub entries: Vec<SceneEntry>,
    /// Ruler segments being measured.
    pub ruler_path: Option<Vec<GeoPoint>>,
    /// Planned route geometry.
    pub route_path: Option<Vec<GeoPoint>>,
    /// Live drawing preview (not yet committed to the library).
    pub preview: Option<PlacemarkGeometry>,
    /// Distance rings around a point: (center, spacing meters, count).
    pub rings: Option<DistanceRings>,
    /// Ground overlays drawn without their image texture.
    pub show_ground_overlay_fill: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DistanceRings {
    pub center: GeoPoint,
    pub spacing_meters: f64,
    pub count: usize,
}

impl Scene {
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
            && self.ruler_path.is_none()
            && self.route_path.is_none()
            && self.preview.is_none()
            && self.rings.is_none()
    }
}

/// Colour used for transient geometry (preview, ruler, route) so it stands out
/// from the library's own styling.
pub const TRANSIENT_COLOR: &str = "#ffd166";
pub const ROUTE_COLOR: &str = "#4cc9f0";

pub fn transient_style(color: &str, width: f64) -> Style {
    Style {
        outline_enabled: true,
        outline_color: color.to_string(),
        outline_width: width,
        filled: false,
        fill_color: color.to_string(),
        fill_opacity: 0.25,
        ..Style::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_scene_reports_empty() {
        assert!(Scene::default().is_empty());
        let scene = Scene {
            preview: Some(
                PlacemarkGeometry::point(GeoPoint::new(1.0, 2.0).expect("valid point"))
                    .expect("geometry"),
            ),
            ..Default::default()
        };
        assert!(!scene.is_empty());
    }
}
