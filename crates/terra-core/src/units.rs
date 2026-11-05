//! Units, coordinate formatting and area/distance formatting.
//!
//! Ported from `packages/core/src/domain/units.ts`. The numeric formatting is
//! deliberately not `format!("{:.2}")`: JavaScript's `toFixed` rounds exact
//! decimal ties away from zero, while Rust's formatter rounds ties to even, so
//! `0.125.toFixed(2)` is `"0.13"` in JS and `"0.12"` in Rust.

use serde::{Deserialize, Serialize};

use crate::domain::geometry::GeoPoint;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum UnitSystem {
    #[default]
    Metric,
    Imperial,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum CoordinateFormat {
    #[default]
    Decimal,
    Dms,
}

const METERS_PER_FOOT: f64 = 0.3048;
const METERS_PER_MILE: f64 = 1609.344;
const SQUARE_METERS_PER_SQUARE_FOOT: f64 = 0.092_903_04;
const SQUARE_METERS_PER_ACRE: f64 = 4_046.856_422_4;
const SQUARE_METERS_PER_HECTARE: f64 = 10_000.0;
const SQUARE_METERS_PER_SQUARE_KM: f64 = 1_000_000.0;
const SQUARE_FEET_PER_ACRE: f64 = 43_560.0;
const ACRES_PER_SQUARE_MILE: f64 = 640.0;

/// Formats a number the way JavaScript's `Number.prototype.toFixed` does.
///
/// Rust's formatter rounds exact decimal ties to even, JS rounds them away from
/// zero, and multiplying by a power of ten first (the obvious workaround)
/// rounds 2.675 up because the product lands on an exact tie. So: format with
/// extra digits to detect a true tie, and only then round away from zero.
pub fn js_to_fixed(value: f64, digits: usize) -> String {
    if value.is_nan() {
        return "NaN".to_string();
    }
    if value.is_infinite() {
        return if value.is_sign_negative() {
            "-Infinity".to_string()
        } else {
            "Infinity".to_string()
        };
    }

    let negative = value.is_sign_negative() && value != 0.0;
    let magnitude = value.abs();

    let text = if is_exact_tie(magnitude, digits) {
        round_tie_away_from_zero(magnitude, digits)
    } else {
        format!("{magnitude:.digits$}")
    };

    if negative
        && text
            .trim_start_matches('0')
            .trim_start_matches('.')
            .is_empty()
    {
        return text;
    }
    if negative { format!("-{text}") } else { text }
}

/// A tie means the exact binary value is `k + 0.5` at the requested decimals.
/// A non-tie f64 is at least one ULP away from that, so twenty extra digits are
/// enough to tell them apart without exact decimal arithmetic.
fn is_exact_tie(value: f64, digits: usize) -> bool {
    let precise = format!("{:.*}", digits + 20, value);
    let Some(dot) = precise.find('.') else {
        return false;
    };
    let fraction = &precise[dot + 1..];
    if fraction.len() <= digits + 1 {
        return false;
    }
    let rest = &fraction[digits..];
    rest.starts_with('5') && rest[1..].bytes().all(|b| b == b'0')
}

fn round_tie_away_from_zero(value: f64, digits: usize) -> String {
    let truncated = format!("{value:.digits$}");
    let mut all_digits: String = truncated.chars().filter(|c| *c != '.').collect();
    increment_decimal(&mut all_digits);
    if all_digits.len() <= digits {
        all_digits.insert_str(0, &"0".repeat(digits + 1 - all_digits.len()));
    }
    let split = all_digits.len() - digits;
    let (integer, fraction) = all_digits.split_at(split);
    if digits == 0 {
        integer.to_string()
    } else {
        format!("{integer}.{fraction}")
    }
}

fn increment_decimal(digits: &mut String) {
    let mut bytes = digits.as_bytes().to_vec();
    let mut i = bytes.len();
    loop {
        if i == 0 {
            bytes.insert(0, b'1');
            break;
        }
        i -= 1;
        if bytes[i] == b'9' {
            bytes[i] = b'0';
        } else {
            bytes[i] += 1;
            break;
        }
    }
    *digits = String::from_utf8(bytes).expect("decimal digits are ASCII");
}

pub fn format_distance(meters: f64, system: UnitSystem) -> String {
    if system == UnitSystem::Imperial {
        if meters < METERS_PER_MILE {
            return format!("{} ft", (meters / METERS_PER_FOOT).round());
        }
        return format!("{} mi", js_to_fixed(meters / METERS_PER_MILE, 2));
    }
    if meters < 1000.0 {
        return format!("{} m", meters.round());
    }
    format!("{} km", js_to_fixed(meters / 1000.0, 2))
}

pub fn format_area(square_meters: f64, system: UnitSystem) -> String {
    if system == UnitSystem::Imperial {
        let square_feet = square_meters / SQUARE_METERS_PER_SQUARE_FOOT;
        if square_feet < SQUARE_FEET_PER_ACRE {
            return format!("{} sq ft", square_feet.round());
        }
        let acres = square_meters / SQUARE_METERS_PER_ACRE;
        if acres < ACRES_PER_SQUARE_MILE {
            return format!("{} acres", js_to_fixed(acres, 2));
        }
        return format!("{} sq mi", js_to_fixed(acres / ACRES_PER_SQUARE_MILE, 2));
    }
    if square_meters < SQUARE_METERS_PER_HECTARE {
        return format!("{} m²", square_meters.round());
    }
    if square_meters < SQUARE_METERS_PER_SQUARE_KM {
        return format!(
            "{} ha",
            js_to_fixed(square_meters / SQUARE_METERS_PER_HECTARE, 2)
        );
    }
    format!(
        "{} km²",
        js_to_fixed(square_meters / SQUARE_METERS_PER_SQUARE_KM, 2)
    )
}

pub fn format_duration(seconds: f64) -> String {
    let total_minutes = (seconds / 60.0).round() as i64;
    let hours = total_minutes / 60;
    let minutes = total_minutes % 60;
    if hours == 0 {
        return format!("{minutes} min");
    }
    format!("{hours} h {minutes} min")
}

pub fn meters_to_unit(meters: f64, system: UnitSystem) -> f64 {
    if system == UnitSystem::Imperial {
        meters / METERS_PER_FOOT
    } else {
        meters
    }
}

pub fn unit_to_meters(value: f64, system: UnitSystem) -> f64 {
    if system == UnitSystem::Imperial {
        value * METERS_PER_FOOT
    } else {
        value
    }
}

pub fn distance_unit_label(system: UnitSystem) -> &'static str {
    if system == UnitSystem::Imperial {
        "ft"
    } else {
        "m"
    }
}

fn to_dms(value: f64, positive_letter: &str, negative_letter: &str) -> String {
    let hemisphere = if value >= 0.0 {
        positive_letter
    } else {
        negative_letter
    };
    let abs = value.abs();
    let degrees = abs.floor();
    let minutes_full = (abs - degrees) * 60.0;
    let minutes = minutes_full.floor();
    let seconds = (minutes_full - minutes) * 60.0;
    format!(
        "{degrees}°{minutes}'{}\"{hemisphere}",
        js_to_fixed(seconds, 2)
    )
}

pub fn format_coordinate(point: &GeoPoint, format: CoordinateFormat) -> String {
    match format {
        CoordinateFormat::Dms => format!(
            "{}, {}",
            to_dms(point.lat, "N", "S"),
            to_dms(point.lon, "E", "W")
        ),
        CoordinateFormat::Decimal => format!(
            "{}, {}",
            js_to_fixed(point.lat, 5),
            js_to_fixed(point.lon, 5)
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn js_to_fixed_matches_to_fixed_for_known_cases() {
        // Values captured from Node: `(v).toFixed(2)`.
        let cases = [
            (0.125, "0.13"),
            (-0.125, "-0.13"),
            (1.005, "1.00"),
            (2.675, "2.67"),
            (1.0049, "1.00"),
            (999.6, "999.60"),
            (1234.5678, "1234.57"),
            (0.0001, "0.00"),
            (1.0 / 3.0, "0.33"),
            (0.0, "0.00"),
            (-0.0, "0.00"),
        ];
        for (input, expected) in cases {
            assert_eq!(js_to_fixed(input, 2), expected, "input was {input}");
        }
        assert_eq!(js_to_fixed(0.125, 5), "0.12500");
        assert_eq!(js_to_fixed(1234.5678, 5), "1234.56780");
    }

    #[test]
    fn distance_formatting_matches_the_web_app() {
        assert_eq!(format_distance(999.4, UnitSystem::Metric), "999 m");
        assert_eq!(format_distance(999.6, UnitSystem::Metric), "1000 m");
        assert_eq!(format_distance(1000.0, UnitSystem::Metric), "1.00 km");
        assert_eq!(format_distance(1234.5, UnitSystem::Metric), "1.23 km");
        assert_eq!(format_distance(100.0, UnitSystem::Imperial), "328 ft");
        assert_eq!(format_distance(1000.0, UnitSystem::Imperial), "3281 ft");
        assert_eq!(format_distance(1609.344, UnitSystem::Imperial), "1.00 mi");
    }

    #[test]
    fn area_formatting_matches_the_web_app() {
        assert_eq!(format_area(9999.4, UnitSystem::Metric), "9999 m²");
        assert_eq!(format_area(10_000.0, UnitSystem::Metric), "1.00 ha");
        assert_eq!(format_area(1_000_000.0, UnitSystem::Metric), "1.00 km²");
        assert_eq!(format_area(9.0, UnitSystem::Imperial), "97 sq ft");
        assert_eq!(
            format_area(SQUARE_METERS_PER_ACRE, UnitSystem::Imperial),
            "1.00 acres"
        );
        assert_eq!(
            format_area(SQUARE_METERS_PER_ACRE * 640.0, UnitSystem::Imperial),
            "1.00 sq mi"
        );
    }

    #[test]
    fn duration_formatting_drops_zero_hours() {
        assert_eq!(format_duration(30.0), "1 min");
        assert_eq!(format_duration(45.0), "1 min");
        assert_eq!(format_duration(3600.0), "1 h 0 min");
        assert_eq!(format_duration(5400.0), "1 h 30 min");
    }

    #[test]
    fn coordinate_formatting_uses_decimal_and_dms() {
        let berlin = GeoPoint::new(13.405, 52.52).expect("valid point");
        assert_eq!(
            format_coordinate(&berlin, CoordinateFormat::Decimal),
            "52.52000, 13.40500"
        );
        assert_eq!(
            format_coordinate(&berlin, CoordinateFormat::Dms),
            "52°31'12.00\"N, 13°24'18.00\"E"
        );
        let south_west = GeoPoint::new(-70.5, -33.45).expect("valid point");
        assert_eq!(
            format_coordinate(&south_west, CoordinateFormat::Dms),
            "33°27'0.00\"S, 70°30'0.00\"W"
        );
    }
}
