//! KML colour strings (`aabbggrr`) and CSS colours (`#rrggbb` + opacity).

/// CSS `#rrggbb` plus opacity to KML's `aabbggrr` byte order.
pub fn css_color_to_kml_color(css_hex: &str, opacity: f64) -> String {
    let channel = |range: std::ops::Range<usize>| {
        css_hex
            .get(range)
            .unwrap_or("00")
            .trim_start_matches('#')
            .to_string()
    };
    let (red, green, blue) = (channel(1..3), channel(3..5), channel(5..7));
    let alpha = clamp_byte(opacity * 255.0);
    format!("{alpha:02x}{blue}{green}{red}").to_lowercase()
}

#[derive(Debug, Clone, PartialEq)]
pub struct CssColor {
    pub hex: String,
    pub opacity: f64,
}

/// KML `aabbggrr` to CSS `#rrggbb` and an opacity in `0..=1`.
pub fn kml_color_to_css_color(kml_color: &str) -> CssColor {
    let slice = |range: std::ops::Range<usize>| kml_color.get(range).unwrap_or("00").to_string();
    let alpha = slice(0..2);
    let blue = slice(2..4);
    let green = slice(4..6);
    let red = slice(6..8);
    CssColor {
        hex: format!("#{red}{green}{blue}").to_lowercase(),
        opacity: u8::from_str_radix(&alpha, 16).unwrap_or(0) as f64 / 255.0,
    }
}

fn clamp_byte(value: f64) -> u8 {
    if !value.is_finite() {
        return 0;
    }
    value.round().clamp(0.0, 255.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_css_to_kml_byte_order() {
        assert_eq!(css_color_to_kml_color("#3366cc", 1.0), "ffcc6633");
        assert_eq!(css_color_to_kml_color("#3366cc", 0.0), "00cc6633");
        assert_eq!(css_color_to_kml_color("#ffffff", 0.5), "80ffffff");
    }

    #[test]
    fn converts_kml_to_css() {
        let color = kml_color_to_css_color("ffcc6633");
        assert_eq!(color.hex, "#3366cc");
        assert!((color.opacity - 1.0).abs() < 1e-9);

        let half = kml_color_to_css_color("80ffffff");
        assert!((half.opacity - 128.0 / 255.0).abs() < 1e-9);
    }

    #[test]
    fn round_trips_through_both_representations() {
        for (hex, opacity) in [
            ("#3366cc", 1.0),
            ("#cc6633", 0.6),
            ("#000000", 0.0),
            ("#abcdef", 0.25),
        ] {
            let kml = css_color_to_kml_color(hex, opacity);
            let back = kml_color_to_css_color(&kml);
            assert_eq!(back.hex, hex);
            assert!((back.opacity - opacity).abs() < 1.0 / 255.0 + f64::EPSILON);
        }
    }
}
