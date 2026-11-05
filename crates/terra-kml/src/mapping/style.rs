//! Style <-> KML `<Style>` mapping, including StyleMap highlights.

use terra_core::Style;
use terra_core::StyleHighlight;

use crate::color::{css_color_to_kml_color, kml_color_to_css_color};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct KmlHighlightStyleData {
    pub line_color_kml: Option<String>,
    pub line_width: Option<f64>,
    pub poly_color_kml: Option<String>,
    pub icon_url: Option<String>,
    pub icon_scale: Option<f64>,
    pub label_color_kml: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct KmlStyleData {
    pub id: String,
    pub line_color_kml: String,
    pub line_width: f64,
    pub poly_color_kml: String,
    pub outline_enabled: bool,
    pub filled: bool,
    pub icon_url: Option<String>,
    pub icon_scale: Option<f64>,
    pub label_color_kml: Option<String>,
    pub highlight: Option<KmlHighlightStyleData>,
    pub balloon_text: Option<String>,
    pub balloon_bg_color_kml: Option<String>,
    pub balloon_text_color_kml: Option<String>,
    pub balloon_display_mode: Option<String>,
    pub list_item_type: Option<String>,
    pub item_icon_href: Option<String>,
}

pub fn style_to_kml_data(style: &Style) -> KmlStyleData {
    KmlStyleData {
        id: style.id.clone(),
        line_color_kml: css_color_to_kml_color(&style.outline_color, 1.0),
        line_width: style.outline_width,
        poly_color_kml: css_color_to_kml_color(&style.fill_color, style.fill_opacity),
        outline_enabled: style.outline_enabled,
        filled: style.filled,
        icon_url: style.icon_url.clone(),
        icon_scale: style.icon_scale,
        label_color_kml: style
            .label_color
            .as_deref()
            .map(|color| css_color_to_kml_color(color, 1.0)),
        highlight: style.highlight.as_ref().map(highlight_to_kml_data),
        balloon_text: style.balloon_text.clone(),
        balloon_bg_color_kml: style
            .balloon_bg_color
            .as_deref()
            .map(|color| css_color_to_kml_color(color, 1.0)),
        balloon_text_color_kml: style
            .balloon_text_color
            .as_deref()
            .map(|color| css_color_to_kml_color(color, 1.0)),
        balloon_display_mode: style.balloon_display_mode.clone(),
        list_item_type: style.list_item_type.clone(),
        item_icon_href: style.item_icon_href.clone(),
    }
}

pub fn kml_data_to_style(data: KmlStyleData) -> Style {
    let line = kml_color_to_css_color(&data.line_color_kml);
    let polygon = kml_color_to_css_color(&data.poly_color_kml);
    Style {
        id: data.id,
        outline_color: line.hex,
        outline_width: data.line_width,
        fill_color: polygon.hex,
        fill_opacity: polygon.opacity,
        outline_enabled: data.outline_enabled,
        filled: data.filled,
        icon_url: data.icon_url,
        icon_scale: data.icon_scale,
        marker_icon: None,
        label_color: data
            .label_color_kml
            .as_deref()
            .map(|color| kml_color_to_css_color(color).hex),
        highlight: data.highlight.map(kml_data_to_highlight),
        balloon_text: data.balloon_text,
        balloon_bg_color: data
            .balloon_bg_color_kml
            .as_deref()
            .map(|color| kml_color_to_css_color(color).hex),
        balloon_text_color: data
            .balloon_text_color_kml
            .as_deref()
            .map(|color| kml_color_to_css_color(color).hex),
        balloon_display_mode: data.balloon_display_mode,
        list_item_type: data.list_item_type,
        item_icon_href: data.item_icon_href,
    }
}

fn highlight_to_kml_data(highlight: &StyleHighlight) -> KmlHighlightStyleData {
    KmlHighlightStyleData {
        line_color_kml: highlight
            .outline_color
            .as_deref()
            .map(|color| css_color_to_kml_color(color, 1.0)),
        line_width: highlight.outline_width,
        poly_color_kml: highlight
            .fill_color
            .as_deref()
            .map(|color| css_color_to_kml_color(color, highlight.fill_opacity.unwrap_or(1.0))),
        icon_url: highlight.icon_url.clone(),
        icon_scale: highlight.icon_scale,
        label_color_kml: highlight
            .label_color
            .as_deref()
            .map(|color| css_color_to_kml_color(color, 1.0)),
    }
}

fn kml_data_to_highlight(data: KmlHighlightStyleData) -> StyleHighlight {
    let line = data.line_color_kml.as_deref().map(kml_color_to_css_color);
    let polygon = data.poly_color_kml.as_deref().map(kml_color_to_css_color);
    StyleHighlight {
        outline_color: line.as_ref().map(|color| color.hex.clone()),
        outline_width: data.line_width,
        fill_color: polygon.as_ref().map(|color| color.hex.clone()),
        fill_opacity: polygon.map(|color| color.opacity),
        icon_url: data.icon_url,
        icon_scale: data.icon_scale,
        label_color: data
            .label_color_kml
            .as_deref()
            .map(|color| kml_color_to_css_color(color).hex),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_core::{NewStyle, create_style};

    fn style() -> Style {
        create_style(NewStyle {
            outline_color: "#3366cc".into(),
            outline_width: 2.0,
            fill_color: "#cc6633".into(),
            fill_opacity: 0.6,
            outline_enabled: None,
            filled: Some(true),
            icon_url: Some("pin.png".into()),
            icon_scale: Some(1.5),
            marker_icon: None,
            label_color: Some("#ffffff".into()),
        })
        .expect("style")
    }

    #[test]
    fn style_round_trips_through_kml_data() {
        let original = style();
        let data = style_to_kml_data(&original);
        assert_eq!(data.line_color_kml, "ffcc6633");
        let restored = kml_data_to_style(data);
        assert_eq!(restored.outline_color, original.outline_color);
        assert_eq!(restored.outline_width, original.outline_width);
        assert_eq!(restored.fill_color, original.fill_color);
        assert!((restored.fill_opacity - original.fill_opacity).abs() < 0.01);
        assert_eq!(restored.outline_enabled, original.outline_enabled);
        assert_eq!(restored.filled, original.filled);
        assert_eq!(restored.icon_url, original.icon_url);
        assert_eq!(restored.label_color, original.label_color);
    }

    #[test]
    fn highlight_round_trips() {
        let mut original = style();
        original.highlight = Some(StyleHighlight {
            outline_color: Some("#ff0000".into()),
            outline_width: Some(3.0),
            fill_color: Some("#00ff00".into()),
            fill_opacity: Some(0.5),
            icon_url: None,
            icon_scale: Some(2.0),
            label_color: Some("#000000".into()),
        });
        let data = style_to_kml_data(&original);
        let restored = kml_data_to_style(data);
        let expected = original.highlight.clone().expect("highlight");
        let actual = restored.highlight.clone().expect("highlight");
        assert_eq!(actual.outline_color, expected.outline_color);
        assert_eq!(actual.outline_width, expected.outline_width);
        assert_eq!(actual.fill_color, expected.fill_color);
        // Opacity is quantised to a byte by the KML colour string.
        assert!(
            (actual.fill_opacity.unwrap_or(0.0) - expected.fill_opacity.unwrap_or(0.0)).abs()
                < 0.01
        );
        assert_eq!(actual.icon_scale, expected.icon_scale);
        assert_eq!(actual.label_color, expected.label_color);
    }
}
