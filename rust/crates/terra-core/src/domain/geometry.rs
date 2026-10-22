use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AltitudeMode {
    ClampToGround,
    RelativeToGround,
    Absolute,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeoPoint {
    pub lon: f64,
    pub lat: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub altitude: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub altitude_mode: Option<AltitudeMode>,
}

impl GeoPoint {
    pub const fn new(lon: f64, lat: f64) -> Self {
        Self {
            lon,
            lat,
            altitude: None,
            altitude_mode: None,
        }
    }

    pub const fn with_altitude(lon: f64, lat: f64, altitude: f64) -> Self {
        Self {
            lon,
            lat,
            altitude: Some(altitude),
            altitude_mode: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RectangleBounds {
    pub north: f64,
    pub south: f64,
    pub east: f64,
    pub west: f64,
}

/// Placemark geometry. Serialised with a `type` tag exactly as the
/// TypeScript app stores it, so existing databases and KML round-trips keep
/// working.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PlacemarkGeometry {
    Point {
        coordinates: GeoPoint,
    },
    #[serde(rename_all = "camelCase")]
    Circle {
        center: GeoPoint,
        radius_meters: f64,
    },
    Rectangle {
        north: f64,
        south: f64,
        east: f64,
        west: f64,
    },
    #[serde(rename_all = "camelCase")]
    Polygon {
        outer_ring: Vec<GeoPoint>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        inner_rings: Option<Vec<Vec<GeoPoint>>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tessellate: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        extrude_height: Option<f64>,
    },
    #[serde(rename_all = "camelCase")]
    LineString {
        path: Vec<GeoPoint>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tessellate: Option<bool>,
        /// Parallel to `path` (gx:Track `when` timestamps), ISO 8601.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        timestamps: Option<Vec<String>>,
    },
    #[serde(rename_all = "camelCase")]
    GroundOverlay {
        bounds: RectangleBounds,
        image_url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rotation: Option<f64>,
    },
    #[serde(rename_all = "camelCase")]
    Model {
        position: GeoPoint,
        model_uri: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        scale: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        heading: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tilt: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        roll: Option<f64>,
    },
}

impl PlacemarkGeometry {
    /// The `type` tag as stored in JSON/KML ExtendedData ("Point", "Circle", ...).
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
