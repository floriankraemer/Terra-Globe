use terra_core::GeoPoint;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenPosition {
    pub x: f64,
    pub y: f64,
}

/// Converts a screen position into a ground point on the ellipsoid.
pub trait ScreenPicker {
    fn pick_ground(&self, position: ScreenPosition) -> Option<GeoPoint>;
}
