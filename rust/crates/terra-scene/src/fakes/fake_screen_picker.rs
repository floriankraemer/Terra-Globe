use terra_core::GeoPoint;

use crate::ports::{ScreenPicker, ScreenPosition};

/// Maps screen x/y linearly to lon/lat (`lon = x * scale`, `lat = y * scale`)
/// unless `miss` is set, in which case every pick misses the globe.
#[derive(Debug, Clone, Copy)]
pub struct FakeScreenPicker {
    pub scale: f64,
    pub miss: bool,
}

impl Default for FakeScreenPicker {
    fn default() -> Self {
        Self {
            scale: 0.01,
            miss: false,
        }
    }
}

impl ScreenPicker for FakeScreenPicker {
    fn pick_ground(&self, position: ScreenPosition) -> Option<GeoPoint> {
        (!self.miss).then(|| GeoPoint::new(position.x * self.scale, position.y * self.scale))
    }
}
