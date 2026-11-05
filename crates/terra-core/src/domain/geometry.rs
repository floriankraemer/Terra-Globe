//! Geometry types, mirroring `packages/core/src/domain/geometry.ts`.

use serde::{Deserialize, Serialize};

use super::DomainError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum AltitudeMode {
    #[default]
    ClampToGround,
    RelativeToGround,
    Absolute,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeoPoint {
    pub lon: f64,
    pub lat: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub altitude: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub altitude_mode: Option<AltitudeMode>,
}

impl GeoPoint {
    /// Creates a point after validating that lon/lat are finite and in range.
    pub fn new(lon: f64, lat: f64) -> Result<Self, DomainError> {
        let point = Self {
            lon,
            lat,
            altitude: None,
            altitude_mode: None,
        };
        point.validate()?;
        Ok(point)
    }

    pub fn with_altitude(lon: f64, lat: f64, altitude: f64) -> Result<Self, DomainError> {
        let mut point = Self::new(lon, lat)?;
        if !altitude.is_finite() {
            return Err(DomainError::OutOfRange {
                label: "altitude",
                min: f64::NEG_INFINITY,
                max: f64::INFINITY,
                value: altitude,
            });
        }
        point.altitude = Some(altitude);
        Ok(point)
    }

    pub fn validate(&self) -> Result<(), DomainError> {
        assert_in_range(self.lon, -180.0, 180.0, "longitude")?;
        assert_in_range(self.lat, -90.0, 90.0, "latitude")
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all_fields = "camelCase")]
pub enum PlacemarkGeometry {
    #[serde(rename = "Point")]
    Point { coordinates: GeoPoint },
    #[serde(rename = "Circle")]
    Circle {
        center: GeoPoint,
        radius_meters: f64,
    },
    #[serde(rename = "Rectangle")]
    Rectangle {
        north: f64,
        south: f64,
        east: f64,
        west: f64,
    },
    #[serde(rename = "Polygon")]
    Polygon {
        outer_ring: Vec<GeoPoint>,
        #[serde(skip_serializing_if = "Option::is_none")]
        inner_rings: Option<Vec<Vec<GeoPoint>>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        tessellate: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        extrude_height: Option<f64>,
    },
    #[serde(rename = "LineString")]
    LineString {
        path: Vec<GeoPoint>,
        #[serde(skip_serializing_if = "Option::is_none")]
        tessellate: Option<bool>,
        /// Parallel to `path`, for `gx:Track` timestamps (ISO 8601).
        #[serde(skip_serializing_if = "Option::is_none")]
        timestamps: Option<Vec<String>>,
    },
    #[serde(rename = "GroundOverlay")]
    GroundOverlay {
        bounds: RectangleBounds,
        image_url: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        rotation: Option<f64>,
    },
    #[serde(rename = "Model")]
    Model {
        position: GeoPoint,
        model_uri: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        scale: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        heading: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        tilt: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        roll: Option<f64>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RectangleBounds {
    pub north: f64,
    pub south: f64,
    pub east: f64,
    pub west: f64,
}

impl RectangleBounds {
    pub fn new(north: f64, south: f64, east: f64, west: f64) -> Result<Self, DomainError> {
        let bounds = Self {
            north,
            south,
            east,
            west,
        };
        bounds.validate()?;
        Ok(bounds)
    }

    pub fn validate(&self) -> Result<(), DomainError> {
        assert_in_range(self.north, -90.0, 90.0, "north")?;
        assert_in_range(self.south, -90.0, 90.0, "south")?;
        assert_in_range(self.east, -180.0, 180.0, "east")?;
        assert_in_range(self.west, -180.0, 180.0, "west")?;
        if self.south >= self.north {
            return Err(DomainError::SouthNotBelowNorth {
                south: self.south,
                north: self.north,
            });
        }
        if self.west >= self.east {
            return Err(DomainError::WestNotBelowEast {
                west: self.west,
                east: self.east,
            });
        }
        Ok(())
    }

    pub fn center(&self) -> GeoPoint {
        GeoPoint {
            lon: (self.west + self.east) / 2.0,
            lat: (self.south + self.north) / 2.0,
            altitude: None,
            altitude_mode: None,
        }
    }
}

impl PlacemarkGeometry {
    pub fn point(coordinates: GeoPoint) -> Result<Self, DomainError> {
        coordinates.validate()?;
        Ok(Self::Point { coordinates })
    }

    pub fn circle(center: GeoPoint, radius_meters: f64) -> Result<Self, DomainError> {
        center.validate()?;
        if !radius_meters.is_finite() || radius_meters <= 0.0 {
            return Err(DomainError::InvalidRadius(radius_meters));
        }
        Ok(Self::Circle {
            center,
            radius_meters,
        })
    }

    pub fn rectangle(bounds: RectangleBounds) -> Result<Self, DomainError> {
        bounds.validate()?;
        Ok(Self::Rectangle {
            north: bounds.north,
            south: bounds.south,
            east: bounds.east,
            west: bounds.west,
        })
    }

    pub fn polygon(
        outer_ring: Vec<GeoPoint>,
        inner_rings: Option<Vec<Vec<GeoPoint>>>,
        tessellate: Option<bool>,
        extrude_height: Option<f64>,
    ) -> Result<Self, DomainError> {
        assert_valid_ring(&outer_ring, "outer ring")?;
        if let Some(rings) = &inner_rings {
            for ring in rings {
                assert_valid_ring(ring, "inner ring")?;
            }
        }
        Ok(Self::Polygon {
            outer_ring,
            inner_rings,
            tessellate,
            extrude_height,
        })
    }

    pub fn line_string(
        path: Vec<GeoPoint>,
        tessellate: Option<bool>,
        timestamps: Option<Vec<String>>,
    ) -> Result<Self, DomainError> {
        if path.len() < 2 {
            return Err(DomainError::TooFewPoints {
                label: "path",
                minimum: 2,
                actual: path.len(),
            });
        }
        for point in &path {
            point.validate()?;
        }
        if let Some(timestamps) = &timestamps {
            if timestamps.len() != path.len() {
                return Err(DomainError::TimestampLengthMismatch {
                    timestamps: timestamps.len(),
                    path: path.len(),
                });
            }
        }
        Ok(Self::LineString {
            path,
            tessellate,
            timestamps,
        })
    }

    pub fn ground_overlay(
        bounds: RectangleBounds,
        image_url: String,
        rotation: Option<f64>,
    ) -> Result<Self, DomainError> {
        bounds.validate()?;
        if image_url.trim().is_empty() {
            return Err(DomainError::EmptyImageUrl);
        }
        Ok(Self::GroundOverlay {
            bounds,
            image_url,
            rotation,
        })
    }

    pub fn model(
        position: GeoPoint,
        model_uri: String,
        scale: Option<f64>,
        heading: Option<f64>,
        tilt: Option<f64>,
        roll: Option<f64>,
    ) -> Result<Self, DomainError> {
        position.validate()?;
        if model_uri.trim().is_empty() {
            return Err(DomainError::EmptyModelUri);
        }
        Ok(Self::Model {
            position,
            model_uri,
            scale,
            heading,
            tilt,
            roll,
        })
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Point { .. } => "Point",
            Self::Circle { .. } => "Circle",
            Self::Rectangle { .. } => "Rectangle",
            Self::Polygon { .. } => "Polygon",
            Self::LineString { .. } => "LineString",
            Self::GroundOverlay { .. } => "GroundOverlay",
            Self::Model { .. } => "Model",
        }
    }
}

pub fn assert_in_range(
    value: f64,
    min: f64,
    max: f64,
    label: &'static str,
) -> Result<(), DomainError> {
    if !value.is_finite() || value < min || value > max {
        return Err(DomainError::OutOfRange {
            label,
            min,
            max,
            value,
        });
    }
    Ok(())
}

fn assert_valid_ring(ring: &[GeoPoint], label: &'static str) -> Result<(), DomainError> {
    if ring.len() < 3 {
        return Err(DomainError::TooFewPoints {
            label,
            minimum: 3,
            actual: ring.len(),
        });
    }
    for point in ring {
        point.validate()?;
    }
    Ok(())
}
