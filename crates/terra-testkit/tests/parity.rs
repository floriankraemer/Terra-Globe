//! Differential parity tests: every expectation here was produced by running
//! the TypeScript implementation (see `tools/parity/extract-fixtures.mjs`).

use serde_json::Value;
use terra_core::domain::geometry::{GeoPoint, PlacemarkGeometry};
use terra_core::units::{format_area, format_coordinate, format_distance, format_duration};
use terra_core::{CoordinateFormat, UnitSystem};

fn fixture() -> Value {
    terra_testkit::core_parity()
}

fn parse_system(value: &str) -> UnitSystem {
    match value {
        "imperial" => UnitSystem::Imperial,
        _ => UnitSystem::Metric,
    }
}

fn parse_coordinate_format(value: &str) -> CoordinateFormat {
    match value {
        "dms" => CoordinateFormat::Dms,
        _ => CoordinateFormat::Decimal,
    }
}

fn number(value: &Value, key: &str) -> f64 {
    value
        .get(key)
        .and_then(Value::as_f64)
        .expect("number field")
}

#[test]
fn distance_formatting_matches_the_typescript_goldens() {
    let cases = fixture();
    for case in cases["units"]["distance"].as_array().expect("array") {
        let meters = number(case, "meters");
        let system = parse_system(case["system"].as_str().expect("system"));
        let expected = case["expected"].as_str().expect("expected");
        assert_eq!(
            format_distance(meters, system),
            expected,
            "formatDistance({meters}, {:?})",
            system
        );
    }
}

#[test]
fn area_formatting_matches_the_typescript_goldens() {
    let cases = fixture();
    for case in cases["units"]["area"].as_array().expect("array") {
        let square_meters = number(case, "squareMeters");
        let system = parse_system(case["system"].as_str().expect("system"));
        let expected = case["expected"].as_str().expect("expected");
        assert_eq!(
            format_area(square_meters, system),
            expected,
            "formatArea({square_meters}, {:?})",
            system
        );
    }
}

#[test]
fn duration_and_coordinate_formatting_match_the_typescript_goldens() {
    let cases = fixture();
    for case in cases["units"]["duration"].as_array().expect("array") {
        let seconds = number(case, "seconds");
        let expected = case["expected"].as_str().expect("expected");
        assert_eq!(
            format_duration(seconds),
            expected,
            "formatDuration({seconds})"
        );
    }
    for case in cases["units"]["coordinate"].as_array().expect("array") {
        let lat = number(case, "lat");
        let lon = number(case, "lon");
        let format = parse_coordinate_format(case["format"].as_str().expect("format"));
        let expected = case["expected"].as_str().expect("expected");
        let point = GeoPoint::new(lon, lat).expect("fixture point is valid");
        assert_eq!(
            format_coordinate(&point, format),
            expected,
            "formatCoordinate({lat}, {lon}, {:?})",
            format
        );
    }
}

#[test]
fn validation_messages_match_the_typescript_goldens() {
    let cases = fixture();
    for case in cases["geometryValidation"]
        .as_array()
        .expect("geometry validation cases")
    {
        let kind = case["kind"].as_str().expect("kind");
        let expected = case["expectedError"].as_str();
        let actual = match kind {
            "point-lon-out-of-range" => GeoPoint::new(190.0, 0.0).err().map(|e| e.to_string()),
            "point-lat-out-of-range" => GeoPoint::new(0.0, -91.0).err().map(|e| e.to_string()),
            "circle-zero-radius" => {
                PlacemarkGeometry::circle(GeoPoint::new(0.0, 0.0).expect("valid"), 0.0)
                    .err()
                    .map(|e| e.to_string())
            }
            "rectangle-inverted" => terra_core::RectangleBounds::new(10.0, 20.0, 1.0, 0.0)
                .err()
                .map(|e| e.to_string()),
            "polygon-too-few-points" => PlacemarkGeometry::polygon(
                vec![
                    GeoPoint::new(0.0, 0.0).expect("valid"),
                    GeoPoint::new(1.0, 1.0).expect("valid"),
                ],
                None,
                None,
                None,
            )
            .err()
            .map(|e| e.to_string()),
            "linestring-too-few-points" => PlacemarkGeometry::line_string(
                vec![GeoPoint::new(0.0, 0.0).expect("valid")],
                None,
                None,
            )
            .err()
            .map(|e| e.to_string()),
            other => panic!("unhandled fixture case {other}"),
        };
        assert_eq!(
            actual.as_deref(),
            expected,
            "validation message for {kind} drifted"
        );
    }
}
