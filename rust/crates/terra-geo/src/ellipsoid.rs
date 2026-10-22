use glam::DVec3;

/// A biaxial ellipsoid of revolution (equatorial radius `a`, polar radius `b`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ellipsoid {
    pub a: f64,
    pub b: f64,
}

impl Ellipsoid {
    pub const WGS84: Self = Self {
        a: 6_378_137.0,
        b: 6_356_752.314_245,
    };

    pub fn radii(&self) -> DVec3 {
        DVec3::new(self.a, self.a, self.b)
    }
}
