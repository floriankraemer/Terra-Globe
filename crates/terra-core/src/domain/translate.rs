//! Moving whole geometries, mirroring `geometryTranslate.ts` (used by
//! drag-to-move).

use crate::domain::geometry::{GeoPoint, PlacemarkGeometry};

pub fn translate_geometry(
    geometry: &PlacemarkGeometry,
    delta_lon: f64,
    delta_lat: f64,
) -> PlacemarkGeometry {
    let shift = |point: &GeoPoint| GeoPoint {
        lon: point.lon + delta_lon,
        lat: point.lat + delta_lat,
        altitude: point.altitude,
        altitude_mode: point.altitude_mode,
    };
    match geometry {
        PlacemarkGeometry::Point { coordinates } => PlacemarkGeometry::Point {
            coordinates: shift(coordinates),
        },
        PlacemarkGeometry::Model {
            position,
            model_uri,
            scale,
            heading,
            tilt,
            roll,
        } => PlacemarkGeometry::Model {
            position: shift(position),
            model_uri: model_uri.clone(),
            scale: *scale,
            heading: *heading,
            tilt: *tilt,
            roll: *roll,
        },
        PlacemarkGeometry::Circle {
            center,
            radius_meters,
        } => PlacemarkGeometry::Circle {
            center: shift(center),
            radius_meters: *radius_meters,
        },
        PlacemarkGeometry::Rectangle {
            north,
            south,
            east,
            west,
        } => PlacemarkGeometry::Rectangle {
            north: north + delta_lat,
            south: south + delta_lat,
            east: east + delta_lon,
            west: west + delta_lon,
        },
        PlacemarkGeometry::GroundOverlay {
            bounds,
            image_url,
            rotation,
        } => PlacemarkGeometry::GroundOverlay {
            bounds: crate::RectangleBounds {
                north: bounds.north + delta_lat,
                south: bounds.south + delta_lat,
                east: bounds.east + delta_lon,
                west: bounds.west + delta_lon,
            },
            image_url: image_url.clone(),
            rotation: *rotation,
        },
        PlacemarkGeometry::Polygon {
            outer_ring,
            inner_rings,
            tessellate,
            extrude_height,
        } => PlacemarkGeometry::Polygon {
            outer_ring: outer_ring.iter().map(shift).collect(),
            inner_rings: inner_rings.as_ref().map(|rings| {
                rings
                    .iter()
                    .map(|ring| ring.iter().map(shift).collect())
                    .collect()
            }),
            tessellate: *tessellate,
            extrude_height: *extrude_height,
        },
        PlacemarkGeometry::LineString {
            path,
            tessellate,
            timestamps,
        } => PlacemarkGeometry::LineString {
            path: path.iter().map(shift).collect(),
            tessellate: *tessellate,
            timestamps: timestamps.clone(),
        },
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

    #[test]
    fn translates_every_geometry_type() {
        let delta = (1.0, 0.5);
        let geometries = [
            PlacemarkGeometry::point(point(1.0, 2.0)).expect("point"),
            PlacemarkGeometry::circle(point(1.0, 2.0), 100.0).expect("circle"),
            PlacemarkGeometry::rectangle(
                crate::RectangleBounds::new(3.0, 2.0, 2.0, 1.0).expect("bounds"),
            )
            .expect("rectangle"),
            PlacemarkGeometry::line_string(vec![point(1.0, 2.0), point(1.5, 2.5)], None, None)
                .expect("line"),
            PlacemarkGeometry::polygon(
                vec![point(1.0, 2.0), point(2.0, 2.0), point(2.0, 3.0)],
                None,
                None,
                None,
            )
            .expect("polygon"),
        ];
        for geometry in geometries {
            let moved = translate_geometry(&geometry, delta.0, delta.1);
            let moved_center = crate::domain::measurements::geometry_center(&moved);
            let original_center = crate::domain::measurements::geometry_center(&geometry);
            assert!((moved_center.lon - original_center.lon - delta.0).abs() < 1e-9);
            assert!((moved_center.lat - original_center.lat - delta.1).abs() < 1e-9);
        }
    }
}
