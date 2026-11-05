//! Differential KML parity tests.
//!
//! Expectations were produced by running the TypeScript parser and serializer
//! (`tools/parity/extract-kml-fixtures.mjs`).

use serde_json::Value;
use terra_kml::{SerializeKmlInput, parse_kml, serialize_kml};

fn entries() -> Vec<Value> {
    let fixture: Value = serde_json::from_str(include_str!("../fixtures/kml-parity.json"))
        .expect("kml parity fixture is valid JSON");
    fixture["entries"]
        .as_array()
        .expect("entries array")
        .clone()
}

/// Ids, timestamps and MultiGeometry group markers are generated when the
/// document does not carry them, so they are normalised away before comparison.
fn normalize(value: &Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.iter().map(normalize).collect()),
        Value::Object(map) => {
            let mut out = serde_json::Map::new();
            for (key, child) in map {
                let normalized = match key.as_str() {
                    "id" | "multiGeometryGroup" => Value::String("<id>".to_string()),
                    "createdAt" | "updatedAt" => Value::String("<ts>".to_string()),
                    _ => normalize(child),
                };
                out.insert(key.clone(), normalized);
            }
            Value::Object(out)
        }
        // JavaScript has one number type: `0` and `0.0` are the same value there
        // but serde_json distinguishes integer and float representations.
        Value::Number(number) => match number.as_f64() {
            Some(value) if value.fract() == 0.0 && value.abs() < 9.0e15 => {
                Value::from(value as i64)
            }
            Some(value) => serde_json::Number::from_f64(value)
                .map(Value::Number)
                .unwrap_or(Value::Null),
            None => Value::Number(number.clone()),
        },
        other => other.clone(),
    }
}

#[test]
fn parsing_matches_the_typescript_goldens() {
    for entry in entries() {
        let name = entry["name"].as_str().expect("name");
        let input = entry["input"].as_str().expect("input");
        let expected = normalize(&entry["parsed"]);

        let parsed = parse_kml(input).unwrap_or_else(|error| panic!("{name}: {error}"));
        let actual = normalize(&serde_json::to_value(&parsed).expect("serializable"));

        assert_eq!(
            actual["folders"], expected["folders"],
            "{name}: folders drifted"
        );
        assert_eq!(
            actual["placemarks"], expected["placemarks"],
            "{name}: placemarks drifted"
        );
        assert_eq!(
            actual["styles"], expected["styles"],
            "{name}: styles drifted"
        );
        assert_eq!(
            actual["screenOverlays"], expected["screenOverlays"],
            "{name}: screen overlays drifted"
        );
        assert_eq!(
            actual["networkLinks"], expected["networkLinks"],
            "{name}: network links drifted"
        );
        assert_eq!(
            actual["warnings"], expected["warnings"],
            "{name}: warnings drifted"
        );
    }
}

/// Masks generated ids and timestamps the same way the fixture generator does.
fn normalize_serialized(xml: &str) -> String {
    let ids = regex::Regex::new(r#"id="[^"]+""#).expect("id regex");
    let timestamps =
        regex::Regex::new(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z").expect("time regex");
    let long_decimals = regex::Regex::new(r"-?\d+\.\d{10,}").expect("decimal regex");
    let masked = ids.replace_all(xml, "id=\"<id>\"");
    let masked = timestamps.replace_all(&masked, "<ts>");
    long_decimals
        .replace_all(&masked, |captures: &regex::Captures<'_>| {
            let value: f64 = captures[0].parse().unwrap_or(0.0);
            format!("{value:.9}")
        })
        .into_owned()
}

#[test]
fn serialization_matches_the_typescript_goldens_for_deterministic_documents() {
    // Documents without explicit ids get generated ids in both implementations,
    // so only the synthetic document (every feature carries an id) can be
    // compared byte for byte.
    for entry in entries() {
        let name = entry["name"].as_str().expect("name");
        if name != "synthetic-all-features" {
            continue;
        }
        let parsed = parse_kml(entry["input"].as_str().expect("input")).expect("parses");
        let serialized = serialize_kml(&SerializeKmlInput {
            folders: &parsed.folders,
            placemarks: &parsed.placemarks,
            styles: &parsed.styles,
            screen_overlays: &parsed.screen_overlays,
        })
        .expect("serializes");
        assert_eq!(
            normalize_serialized(&serialized),
            entry["serialized"].as_str().expect("serialized"),
            "{name}: serialized KML drifted"
        );
    }
}

#[test]
fn parse_serialize_parse_is_stable_for_the_corpus() {
    for entry in entries() {
        let name = entry["name"].as_str().expect("name");
        let first = parse_kml(entry["input"].as_str().expect("input")).expect("parses");
        let serialized = serialize_kml(&SerializeKmlInput {
            folders: &first.folders,
            placemarks: &first.placemarks,
            styles: &first.styles,
            screen_overlays: &first.screen_overlays,
        })
        .expect("serializes");
        let second = parse_kml(&serialized).expect("reparses");

        // Styles are excluded deliberately: a StyleMap parses into its map entry
        // *and* its two component styles, and both are written back, so the count
        // grows on every export. The web app behaves the same way; see
        // docs/parity-notes.md for the follow-up.
        let first = normalize(&serde_json::to_value(&first).expect("json"));
        let second = normalize(&serde_json::to_value(&second).expect("json"));
        for field in ["folders", "placemarks", "screenOverlays", "networkLinks"] {
            assert_eq!(
                first[field], second[field],
                "{name}: {field} changed on round trip"
            );
        }
    }
}
