//! Mapping between domain geometry and KML geometry elements.
//!
//! Circle and Rectangle have no KML equivalent, so they export as a polygon
//! approximation plus `<ExtendedData>` markers that let this app reconstruct the
//! exact shape on import (Google Earth shows the plain polygon, like it does for
//! its own rectangles).

use terra_core::GeoPoint;
use terra_core::PlacemarkGeometry;

use crate::KmlError;
use terra_core::circle_to_polygon_ring;

pub const SHAPE_KEY: &str = "terra-globe:shape";
pub const CIRCLE_LON_KEY: &str = "terra-globe:centerLon";
pub const CIRCLE_LAT_KEY: &str = "terra-globe:centerLat";
pub const CIRCLE_RADIUS_KEY: &str = "terra-globe:radiusMeters";
pub const RECTANGLE_NORTH_KEY: &str = "terra-globe:north";
pub const RECTANGLE_SOUTH_KEY: &str = "terra-globe:south";
pub const RECTANGLE_EAST_KEY: &str = "terra-globe:east";
pub const RECTANGLE_WEST_KEY: &str = "terra-globe:west";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KmlGeometryElement {
    Point,
    Polygon,
    LineString,
}

#[derive(Debug, Clone, PartialEq)]
pub struct KmlGeometryResult {
    pub element: KmlGeometryElement,
    /// `[outerRing, ...innerRings]` for polygons, `[[point]]` for points.
    pub rings: Vec<Vec<GeoPoint>>,
    pub extended_data: Vec<(String, String)>,
}

/// Serialises a shape geometry. `Model` and `GroundOverlay` are different KML
/// constructs and are handled by the serializer directly.
pub fn domain_geometry_to_kml(geometry: &PlacemarkGeometry) -> Result<KmlGeometryResult, KmlError> {
    Ok(match geometry {
        PlacemarkGeometry::Point { coordinates } => KmlGeometryResult {
            element: KmlGeometryElement::Point,
            rings: vec![vec![*coordinates]],
            extended_data: Vec::new(),
        },
        PlacemarkGeometry::Polygon {
            outer_ring,
            inner_rings,
            ..
        } => {
            let mut rings = vec![outer_ring.clone()];
            rings.extend(inner_rings.clone().unwrap_or_default());
            KmlGeometryResult {
                element: KmlGeometryElement::Polygon,
                rings,
                extended_data: Vec::new(),
            }
        }
        PlacemarkGeometry::LineString { path, .. } => KmlGeometryResult {
            element: KmlGeometryElement::LineString,
            rings: vec![path.clone()],
            extended_data: Vec::new(),
        },
        PlacemarkGeometry::Rectangle {
            north,
            south,
            east,
            west,
        } => KmlGeometryResult {
            element: KmlGeometryElement::Polygon,
            rings: vec![rectangle_ring(*north, *south, *east, *west)],
            extended_data: vec![
                (SHAPE_KEY.to_string(), "rectangle".to_string()),
                (RECTANGLE_NORTH_KEY.to_string(), number(*north)),
                (RECTANGLE_SOUTH_KEY.to_string(), number(*south)),
                (RECTANGLE_EAST_KEY.to_string(), number(*east)),
                (RECTANGLE_WEST_KEY.to_string(), number(*west)),
            ],
        },
        PlacemarkGeometry::Circle {
            center,
            radius_meters,
        } => KmlGeometryResult {
            element: KmlGeometryElement::Polygon,
            rings: vec![circle_to_polygon_ring(center, *radius_meters, 64)],
            extended_data: vec![
                (SHAPE_KEY.to_string(), "circle".to_string()),
                (CIRCLE_LON_KEY.to_string(), number(center.lon)),
                (CIRCLE_LAT_KEY.to_string(), number(center.lat)),
                (CIRCLE_RADIUS_KEY.to_string(), number(*radius_meters)),
            ],
        },
        PlacemarkGeometry::GroundOverlay { .. } | PlacemarkGeometry::Model { .. } => {
            return Err(KmlError::UnsupportedGeometry(
                geometry.type_name().to_string(),
            ));
        }
    })
}

/// Reconstructs a shape geometry from parsed rings plus ExtendedData.
pub fn kml_to_domain_geometry(
    element: KmlGeometryElement,
    rings: &[Vec<GeoPoint>],
    extended_data: &std::collections::BTreeMap<String, String>,
) -> Result<PlacemarkGeometry, KmlError> {
    match extended_data.get(SHAPE_KEY).map(String::as_str) {
        Some("rectangle") => {
            let north = required_number(extended_data, RECTANGLE_NORTH_KEY)?;
            let south = required_number(extended_data, RECTANGLE_SOUTH_KEY)?;
            let east = required_number(extended_data, RECTANGLE_EAST_KEY)?;
            let west = required_number(extended_data, RECTANGLE_WEST_KEY)?;
            let bounds = terra_core::RectangleBounds::new(north, south, east, west)
                .map_err(|error| KmlError::InvalidGeometry(error.to_string()))?;
            PlacemarkGeometry::rectangle(bounds)
                .map_err(|error| KmlError::InvalidGeometry(error.to_string()))
        }
        Some("circle") => {
            let lon = required_number(extended_data, CIRCLE_LON_KEY)?;
            let lat = required_number(extended_data, CIRCLE_LAT_KEY)?;
            let radius = required_number(extended_data, CIRCLE_RADIUS_KEY)?;
            let center = GeoPoint::new(lon, lat)
                .map_err(|error| KmlError::InvalidGeometry(error.to_string()))?;
            PlacemarkGeometry::circle(center, radius)
                .map_err(|error| KmlError::InvalidGeometry(error.to_string()))
        }
        _ => match element {
            KmlGeometryElement::Point => {
                let point = rings
                    .first()
                    .and_then(|ring| ring.first())
                    .copied()
                    .ok_or_else(|| {
                        KmlError::InvalidGeometry("Point has no coordinates".to_string())
                    })?;
                PlacemarkGeometry::point(point)
                    .map_err(|error| KmlError::InvalidGeometry(error.to_string()))
            }
            KmlGeometryElement::LineString => {
                let path = rings.first().cloned().ok_or_else(|| {
                    KmlError::InvalidGeometry("LineString has no coordinates".to_string())
                })?;
                PlacemarkGeometry::line_string(path, None, None)
                    .map_err(|error| KmlError::InvalidGeometry(error.to_string()))
            }
            KmlGeometryElement::Polygon => {
                let mut rings = rings.to_vec();
                if rings.is_empty() {
                    return Err(KmlError::InvalidGeometry(
                        "Polygon has no coordinates".to_string(),
                    ));
                }
                let outer = rings.remove(0);
                let inner = if rings.is_empty() { None } else { Some(rings) };
                PlacemarkGeometry::polygon(outer, inner, None, None)
                    .map_err(|error| KmlError::InvalidGeometry(error.to_string()))
            }
        },
    }
}

fn required_number(
    extended_data: &std::collections::BTreeMap<String, String>,
    key: &str,
) -> Result<f64, KmlError> {
    extended_data
        .get(key)
        .and_then(|value| value.parse::<f64>().ok())
        .ok_or_else(|| KmlError::InvalidGeometry(format!("missing or invalid {key}")))
}

fn rectangle_ring(north: f64, south: f64, east: f64, west: f64) -> Vec<GeoPoint> {
    let point = |lon: f64, lat: f64| GeoPoint {
        lon,
        lat,
        altitude: None,
        altitude_mode: None,
    };
    vec![
        point(west, south),
        point(east, south),
        point(east, north),
        point(west, north),
        point(west, south),
    ]
}

fn number(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < 1e15 {
        format!("{}", value as i64)
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(lon: f64, lat: f64) -> GeoPoint {
        GeoPoint {
            lon,
            lat,
            altitude: None,
            altitude_mode: None,
        }
    }

    fn extended(entries: &[(&str, &str)]) -> std::collections::BTreeMap<String, String> {
        entries
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect()
    }

    #[test]
    fn rectangle_exports_as_a_closed_polygon_with_round_trip_markers() {
        let geometry = PlacemarkGeometry::rectangle(
            terra_core::RectangleBounds::new(52.6, 52.4, 13.5, 13.3).expect("bounds"),
        )
        .expect("geometry");
        let result = domain_geometry_to_kml(&geometry).expect("maps");
        assert_eq!(result.element, KmlGeometryElement::Polygon);
        assert_eq!(result.rings[0].len(), 5);
        let markers: std::collections::BTreeMap<_, _> =
            result.extended_data.iter().cloned().collect();
        assert_eq!(
            markers.get(SHAPE_KEY).map(String::as_str),
            Some("rectangle")
        );
        let restored = kml_to_domain_geometry(KmlGeometryElement::Polygon, &result.rings, &markers)
            .expect("restores");
        assert_eq!(restored, geometry);
    }

    #[test]
    fn circle_exports_as_a_ring_and_restores_exactly() {
        let geometry = PlacemarkGeometry::circle(point(13.405, 52.52), 1_200.0).expect("geometry");
        let result = domain_geometry_to_kml(&geometry).expect("maps");
        assert_eq!(result.element, KmlGeometryElement::Polygon);
        assert_eq!(result.rings[0].len(), 65);
        let markers: std::collections::BTreeMap<_, _> =
            result.extended_data.iter().cloned().collect();
        let restored = kml_to_domain_geometry(KmlGeometryElement::Polygon, &result.rings, &markers)
            .expect("restores");
        assert_eq!(restored, geometry);
    }

    #[test]
    fn plain_polygon_keeps_holes() {
        let outer = vec![point(0.0, 0.0), point(1.0, 0.0), point(1.0, 1.0)];
        let inner = vec![point(0.2, 0.2), point(0.4, 0.2), point(0.4, 0.4)];
        let geometry =
            PlacemarkGeometry::polygon(outer, Some(vec![inner]), Some(true), Some(100.0))
                .expect("geometry");
        let result = domain_geometry_to_kml(&geometry).expect("maps");
        assert_eq!(result.rings.len(), 2);
        let restored = kml_to_domain_geometry(
            KmlGeometryElement::Polygon,
            &result.rings,
            &std::collections::BTreeMap::new(),
        )
        .expect("restores");
        match restored {
            PlacemarkGeometry::Polygon {
                outer_ring,
                inner_rings,
                ..
            } => {
                assert_eq!(outer_ring.len(), 3);
                assert_eq!(inner_rings.map(|rings| rings.len()), Some(1));
            }
            other => panic!("unexpected geometry {other:?}"),
        }
    }

    #[test]
    fn model_and_ground_overlay_are_not_shape_geometries() {
        let model = PlacemarkGeometry::model(
            point(0.0, 0.0),
            "model.glb".to_string(),
            None,
            None,
            None,
            None,
        )
        .expect("model");
        assert!(matches!(
            domain_geometry_to_kml(&model),
            Err(KmlError::UnsupportedGeometry(_))
        ));
    }

    #[test]
    fn missing_round_trip_markers_are_an_explicit_error() {
        let error = kml_to_domain_geometry(
            KmlGeometryElement::Polygon,
            &[vec![point(0.0, 0.0), point(1.0, 0.0), point(1.0, 1.0)]],
            &extended(&[(SHAPE_KEY, "rectangle")]),
        )
        .expect_err("missing bounds");
        assert!(matches!(error, KmlError::InvalidGeometry(_)));
    }
}
