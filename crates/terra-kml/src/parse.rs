//! KML reader, ported from `packages/core/src/kml/parseKml.ts`.
//!
//! Import is deliberately fail-soft: an unsupported or malformed feature is
//! skipped with a warning instead of failing the whole document.

use std::collections::BTreeMap;

use terra_core::domain::folder::{Folder, NewFolder, create_folder};
use terra_core::domain::geometry::{GeoPoint, PlacemarkGeometry};
use terra_core::domain::placemark::{
    NewPlacemark, Placemark, PlacemarkRegion, PlacemarkView, ViewKind, create_placemark,
};
use terra_core::domain::screen_overlay::{
    AnchorUnits, NewScreenOverlay, ScreenOverlay, ScreenOverlayAnchor, create_screen_overlay,
};
use terra_core::domain::style::{Style, StyleHighlight};

use crate::KmlError;
use crate::mapping::geometry::{KmlGeometryElement, kml_to_domain_geometry};
use crate::mapping::style::{KmlStyleData, kml_data_to_style};
use crate::xml::{Element, parse_document};

pub const RESERVED_EXTENDED_DATA_PREFIX: &str = "terra-globe:";

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KmlNetworkLinkRef {
    pub folder_id: Option<String>,
    pub href: String,
}

#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParseKmlResult {
    pub folders: Vec<Folder>,
    pub placemarks: Vec<Placemark>,
    pub styles: Vec<Style>,
    pub screen_overlays: Vec<ScreenOverlay>,
    pub network_links: Vec<KmlNetworkLinkRef>,
    /// Human-readable notes about skipped or downgraded features.
    pub warnings: Vec<String>,
}

#[derive(Default)]
struct State {
    folders: Vec<Folder>,
    placemarks: Vec<Placemark>,
    screen_overlays: Vec<ScreenOverlay>,
    network_links: Vec<KmlNetworkLinkRef>,
    warnings: Vec<String>,
}

pub fn parse_kml(xml: &str) -> Result<ParseKmlResult, KmlError> {
    let root = parse_document(xml)?;
    if !root.matches("kml") {
        return Err(KmlError::Xml("no root <kml> element found".to_string()));
    }
    let container = root.child("Document").unwrap_or(&root);
    let mut state = State::default();

    let base_styles: Vec<Style> = container
        .children_named("Style")
        .map(|node| parse_style_node(node, node.attr("id").unwrap_or_default()))
        .collect();
    let mut styles_by_id: BTreeMap<String, Style> = base_styles
        .iter()
        .map(|style| (style.id.clone(), style.clone()))
        .collect();
    let mut styles = base_styles;
    for style_map in container.children_named("StyleMap") {
        let style = parse_style_map(style_map, &styles_by_id);
        if let Some(style) = style {
            styles_by_id.insert(style.id.clone(), style.clone());
            styles.push(style);
        }
    }

    for folder in container.children_named("Folder") {
        parse_folder(folder, None, &mut state);
    }
    for node in container.children_named("Placemark") {
        state
            .placemarks
            .extend(parse_placemarks(node, None, &mut state.warnings));
    }
    for node in container.children_named("GroundOverlay") {
        state
            .placemarks
            .extend(parse_ground_overlay(node, None, &mut state.warnings));
    }
    for node in container.children_named("PhotoOverlay") {
        state
            .placemarks
            .extend(parse_photo_overlay(node, None, &mut state.warnings));
    }
    for node in container.children_named("ScreenOverlay") {
        state
            .screen_overlays
            .extend(parse_screen_overlay(node, None, &mut state.warnings));
    }
    state.network_links.extend(parse_network_link_refs(
        container.children_named("NetworkLink"),
        None,
    ));

    Ok(ParseKmlResult {
        folders: state.folders,
        placemarks: state.placemarks,
        styles,
        screen_overlays: state.screen_overlays,
        network_links: state.network_links,
        warnings: state.warnings,
    })
}

fn parse_folder(node: &Element, parent_id: Option<&str>, state: &mut State) {
    let extended_data = parse_extended_data(node);
    let folder = create_folder(NewFolder {
        name: node.child_text("name").unwrap_or_default(),
        parent_id: parent_id.map(str::to_string),
        order: extended_data
            .get("terra-globe:order")
            .and_then(|value| value.parse::<i64>().ok())
            .unwrap_or(0),
        visibility: Some(visibility_of(node)),
        description: node.child_text("description"),
        open: node.child_text("open").map(|value| value == "1"),
    });
    let Ok(mut folder) = folder else {
        state.warnings.push(format!(
            "Skipped folder \"{}\": invalid folder data.",
            node.child_text("name").unwrap_or_default()
        ));
        return;
    };
    apply_meta(
        &mut folder.id,
        &mut folder.created_at,
        &mut folder.updated_at,
        node,
        &extended_data,
    );
    let resolved_id = folder.id.clone();
    state.folders.push(folder);

    for child in node.children_named("Folder") {
        parse_folder(child, Some(&resolved_id), state);
    }
    for child in node.children_named("Placemark") {
        state.placemarks.extend(parse_placemarks(
            child,
            Some(&resolved_id),
            &mut state.warnings,
        ));
    }
    for child in node.children_named("GroundOverlay") {
        state.placemarks.extend(parse_ground_overlay(
            child,
            Some(&resolved_id),
            &mut state.warnings,
        ));
    }
    for child in node.children_named("PhotoOverlay") {
        state.placemarks.extend(parse_photo_overlay(
            child,
            Some(&resolved_id),
            &mut state.warnings,
        ));
    }
    for child in node.children_named("ScreenOverlay") {
        state.screen_overlays.extend(parse_screen_overlay(
            child,
            Some(&resolved_id),
            &mut state.warnings,
        ));
    }
    state.network_links.extend(parse_network_link_refs(
        node.children_named("NetworkLink"),
        Some(&resolved_id),
    ));
}

fn parse_placemarks(
    node: &Element,
    folder_id: Option<&str>,
    warnings: &mut Vec<String>,
) -> Vec<Placemark> {
    let name = node.child_text("name").unwrap_or_default();

    if let Some(model) = node.child("Model") {
        let extended_data = parse_extended_data(node);
        let Some(geometry) = parse_model_node(model, &name, warnings) else {
            warnings.push(format!(
                "Skipped placemark \"{name}\": Model has no Link href."
            ));
            return Vec::new();
        };
        let input = NewPlacemark {
            name,
            folder_id: folder_id.map(str::to_string),
            description: node.child_text("description"),
            style_id: style_id_of(node),
            visibility: Some(visibility_of(node)),
            extended_data: non_reserved_extended_data(&extended_data),
            ..feature_extras(node)
        };
        let Ok(mut placemark) = create_placemark(input, geometry) else {
            warnings.push(format!(
                "Skipped placemark \"{}\": invalid placemark.",
                node.child_text("name").unwrap_or_default()
            ));
            return Vec::new();
        };
        apply_meta(
            &mut placemark.id,
            &mut placemark.created_at,
            &mut placemark.updated_at,
            node,
            &extended_data,
        );
        return vec![placemark];
    }

    let geometries = parse_geometries(node);
    if geometries.is_empty() {
        warnings.push(format!(
            "Skipped placemark \"{name}\": no supported geometry (Point/Polygon) found."
        ));
        return Vec::new();
    }

    let extended_data = parse_extended_data(node);
    let style_id = style_id_of(node);
    let multi_geometry_group = (geometries.len() > 1).then(|| {
        node.attr("id")
            .map(str::to_string)
            .unwrap_or_else(|| format!("multigeom-{name}"))
    });
    let arbitrary_extended_data = non_reserved_extended_data(&extended_data);
    let extras = feature_extras(node);
    let node_id = node.attr("id").map(str::to_string);

    let mut placemarks = Vec::new();
    for (index, parsed) in geometries.iter().enumerate() {
        let entry_name = if geometries.len() > 1 && index > 0 {
            format!("{name} ({})", index + 1)
        } else {
            name.clone()
        };
        let geometry = match kml_to_domain_geometry(parsed.element, &parsed.rings, &extended_data) {
            Ok(mut geometry) => {
                match &mut geometry {
                    PlacemarkGeometry::LineString {
                        tessellate,
                        timestamps,
                        ..
                    } => {
                        if parsed.tessellate {
                            *tessellate = Some(true);
                        }
                        if parsed.timestamps.is_some() {
                            *timestamps = parsed.timestamps.clone();
                        }
                    }
                    PlacemarkGeometry::Polygon {
                        tessellate,
                        extrude_height,
                        ..
                    } => {
                        if parsed.tessellate {
                            *tessellate = Some(true);
                        }
                        if parsed.extrude_height.is_some() {
                            *extrude_height = parsed.extrude_height;
                        }
                    }
                    _ => {}
                }
                geometry
            }
            Err(error) => {
                warnings.push(format!("Skipped placemark \"{entry_name}\": {error}."));
                continue;
            }
        };

        let input = NewPlacemark {
            name: entry_name.clone(),
            folder_id: folder_id.map(str::to_string),
            description: node.child_text("description"),
            style_id: style_id.clone(),
            visibility: Some(visibility_of(node)),
            extended_data: arbitrary_extended_data.clone(),
            multi_geometry_group: multi_geometry_group.clone(),
            ..extras.clone()
        };
        let placemark = match create_placemark(input, geometry) {
            Ok(placemark) => placemark,
            Err(error) => {
                warnings.push(format!("Skipped placemark \"{entry_name}\": {error}."));
                continue;
            }
        };
        let mut placemark = placemark;
        let base_id = node_id.clone().unwrap_or_else(|| placemark.id.clone());
        placemark.id = if geometries.len() > 1 {
            format!("{base_id}-{index}")
        } else {
            base_id
        };
        if let Some(created_at) = extended_data.get("terra-globe:createdAt") {
            placemark.created_at = created_at.clone();
        }
        if let Some(updated_at) = extended_data.get("terra-globe:updatedAt") {
            placemark.updated_at = updated_at.clone();
        }
        placemarks.push(placemark);
    }
    placemarks
}

fn parse_model_node(
    model: &Element,
    name: &str,
    warnings: &mut Vec<String>,
) -> Option<PlacemarkGeometry> {
    let location = model.child("Location");
    let orientation = model.child("Orientation");
    let scale = model.child("Scale");
    let model_uri = model
        .child("Link")
        .and_then(|link| link.child_text("href"))
        .or_else(|| model.child_text("href"))?;

    let lon = location
        .and_then(|node| node.child_f64("longitude"))
        .unwrap_or(0.0);
    let lat = location
        .and_then(|node| node.child_f64("latitude"))
        .unwrap_or(0.0);
    let altitude = location.and_then(|node| node.child_f64("altitude"));
    let position = match altitude {
        Some(altitude) => GeoPoint::with_altitude(lon, lat, altitude),
        None => GeoPoint::new(lon, lat),
    };
    let position = match position {
        Ok(position) => position,
        Err(error) => {
            warnings.push(format!("Skipped placemark \"{name}\": {error}."));
            return None;
        }
    };

    match PlacemarkGeometry::model(
        position,
        model_uri,
        scale.and_then(|node| node.child_f64("x")),
        orientation.and_then(|node| node.child_f64("heading")),
        orientation.and_then(|node| node.child_f64("tilt")),
        orientation.and_then(|node| node.child_f64("roll")),
    ) {
        Ok(geometry) => Some(geometry),
        Err(error) => {
            warnings.push(format!("Skipped placemark \"{name}\": {error}."));
            None
        }
    }
}

fn parse_ground_overlay(
    node: &Element,
    folder_id: Option<&str>,
    warnings: &mut Vec<String>,
) -> Vec<Placemark> {
    let name = node.child_text("name").unwrap_or_default();
    let Some(box_node) = node.child("LatLonBox") else {
        warnings.push(format!(
            "Skipped GroundOverlay \"{name}\": missing LatLonBox or Icon href."
        ));
        return Vec::new();
    };
    let Some(href) = node.child("Icon").and_then(|icon| icon.child_text("href")) else {
        warnings.push(format!(
            "Skipped GroundOverlay \"{name}\": missing LatLonBox or Icon href."
        ));
        return Vec::new();
    };

    let bounds = (
        box_node.child_f64("north"),
        box_node.child_f64("south"),
        box_node.child_f64("east"),
        box_node.child_f64("west"),
    );
    let (Some(north), Some(south), Some(east), Some(west)) = bounds else {
        warnings.push(format!(
            "Skipped GroundOverlay \"{name}\": LatLonBox is incomplete."
        ));
        return Vec::new();
    };

    let extended_data = parse_extended_data(node);
    let opacity = overlay_opacity(node, &name, warnings);
    let geometry = terra_core::RectangleBounds::new(north, south, east, west)
        .map_err(|error| error.to_string())
        .and_then(|bounds| {
            PlacemarkGeometry::ground_overlay(bounds, href, box_node.child_f64("rotation"), opacity)
                .map_err(|error| error.to_string())
        });
    let geometry = match geometry {
        Ok(geometry) => geometry,
        Err(error) => {
            warnings.push(format!("Skipped GroundOverlay \"{name}\": {error}."));
            return Vec::new();
        }
    };

    let input = NewPlacemark {
        name,
        folder_id: folder_id.map(str::to_string),
        description: node.child_text("description"),
        visibility: Some(visibility_of(node)),
        extended_data: non_reserved_extended_data(&extended_data),
        ..feature_extras(node)
    };
    match create_placemark(input, geometry) {
        Ok(mut placemark) => {
            apply_meta(
                &mut placemark.id,
                &mut placemark.created_at,
                &mut placemark.updated_at,
                node,
                &extended_data,
            );
            vec![placemark]
        }
        Err(error) => {
            warnings.push(format!(
                "Skipped GroundOverlay \"{}\": {error}.",
                node.child_text("name").unwrap_or_default()
            ));
            Vec::new()
        }
    }
}

/// Alpha of the overlay's `<color>` (`aabbggrr`) as `0.0..=1.0`. A malformed
/// colour is ignored with a warning rather than hiding the whole overlay.
fn overlay_opacity(node: &Element, name: &str, warnings: &mut Vec<String>) -> Option<f64> {
    let color = node.child_text("color")?;
    let valid = color.len() == 8 && color.chars().all(|c| c.is_ascii_hexdigit());
    if !valid {
        warnings.push(format!(
            "Ignored malformed <color> \"{color}\" on GroundOverlay \"{name}\"."
        ));
        return None;
    }
    let alpha = u8::from_str_radix(&color[..2], 16).ok()?;
    Some(f64::from(alpha) / 255.0)
}

fn parse_photo_overlay(
    node: &Element,
    folder_id: Option<&str>,
    warnings: &mut Vec<String>,
) -> Vec<Placemark> {
    if node.child("LatLonBox").is_some() {
        return parse_ground_overlay(node, folder_id, warnings);
    }
    let name = node.child_text("name").unwrap_or_default();
    let href = node.child("Icon").and_then(|icon| icon.child_text("href"));
    let point = node
        .child("Point")
        .and_then(|point| point.child_text("coordinates"))
        .and_then(|text| parse_coordinates(&text).into_iter().next());
    let (Some(href), Some(point)) = (href, point) else {
        warnings.push(format!(
            "Skipped PhotoOverlay \"{name}\": no LatLonBox and no Point/Icon to fall back to."
        ));
        return Vec::new();
    };

    let extended_data = parse_extended_data(node);
    let Ok(geometry) = PlacemarkGeometry::point(point) else {
        warnings.push(format!("Skipped PhotoOverlay \"{name}\": invalid Point."));
        return Vec::new();
    };
    let mut arbitrary = non_reserved_extended_data(&extended_data).unwrap_or_default();
    arbitrary.insert("terra-globe:photoUrl".to_string(), href);

    let input = NewPlacemark {
        name,
        folder_id: folder_id.map(str::to_string),
        description: node.child_text("description"),
        visibility: Some(visibility_of(node)),
        extended_data: Some(arbitrary),
        ..feature_extras(node)
    };
    match create_placemark(input, geometry) {
        Ok(mut placemark) => {
            apply_meta(
                &mut placemark.id,
                &mut placemark.created_at,
                &mut placemark.updated_at,
                node,
                &extended_data,
            );
            vec![placemark]
        }
        Err(error) => {
            warnings.push(format!(
                "Skipped PhotoOverlay \"{}\": {error}.",
                node.child_text("name").unwrap_or_default()
            ));
            Vec::new()
        }
    }
}

fn parse_screen_overlay(
    node: &Element,
    folder_id: Option<&str>,
    warnings: &mut Vec<String>,
) -> Vec<ScreenOverlay> {
    let name = node.child_text("name").unwrap_or_default();
    let Some(image_url) = node.child("Icon").and_then(|icon| icon.child_text("href")) else {
        warnings.push(format!(
            "Skipped ScreenOverlay \"{name}\": missing Icon href."
        ));
        return Vec::new();
    };
    let size = node.child("size");
    let input = NewScreenOverlay {
        name: name.clone(),
        folder_id: folder_id.map(str::to_string),
        image_url,
        overlay_xy: parse_anchor(node.child("overlayXY"), 0.0, 1.0),
        screen_xy: parse_anchor(node.child("screenXY"), 0.0, 1.0),
        size_x: size
            .and_then(|size| size.attr("x"))
            .and_then(|value| value.parse().ok()),
        size_y: size
            .and_then(|size| size.attr("y"))
            .and_then(|value| value.parse().ok()),
        rotation: node.child_f64("rotation"),
        visibility: Some(visibility_of(node)),
        order: None,
    };
    match create_screen_overlay(input) {
        Ok(overlay) => vec![overlay],
        Err(error) => {
            warnings.push(format!("Skipped ScreenOverlay \"{name}\": {error}."));
            Vec::new()
        }
    }
}

fn parse_anchor(node: Option<&Element>, default_x: f64, default_y: f64) -> ScreenOverlayAnchor {
    let units = |value: Option<&str>| match value {
        Some("pixels") => AnchorUnits::Pixels,
        Some("insetPixels") => AnchorUnits::InsetPixels,
        _ => AnchorUnits::Fraction,
    };
    ScreenOverlayAnchor {
        x: node
            .and_then(|node| node.attr("x"))
            .and_then(|value| value.parse().ok())
            .unwrap_or(default_x),
        y: node
            .and_then(|node| node.attr("y"))
            .and_then(|value| value.parse().ok())
            .unwrap_or(default_y),
        x_units: units(node.and_then(|node| node.attr("xunits"))),
        y_units: units(node.and_then(|node| node.attr("yunits"))),
    }
}

fn parse_network_link_refs<'a>(
    nodes: impl Iterator<Item = &'a Element>,
    folder_id: Option<&str>,
) -> Vec<KmlNetworkLinkRef> {
    nodes
        .filter_map(|node| {
            let href = node
                .child("Link")
                .and_then(|link| link.child_text("href"))
                .or_else(|| node.child("Url").and_then(|url| url.child_text("href")))?;
            Some(KmlNetworkLinkRef {
                folder_id: folder_id.map(str::to_string),
                href,
            })
        })
        .collect()
}

fn parse_style_node(node: &Element, id: &str) -> Style {
    let line = node.child("LineStyle");
    let polygon = node.child("PolyStyle");
    let icon = node.child("IconStyle");
    let label = node.child("LabelStyle");
    let balloon = node.child("BalloonStyle");
    let list = node.child("ListStyle");

    kml_data_to_style(KmlStyleData {
        id: id.to_string(),
        line_color_kml: line
            .and_then(|line| line.child_text("color"))
            .unwrap_or_else(|| "ff000000".to_string()),
        line_width: line.and_then(|line| line.child_f64("width")).unwrap_or(1.0),
        poly_color_kml: polygon
            .and_then(|polygon| polygon.child_text("color"))
            .unwrap_or_else(|| "ff000000".to_string()),
        outline_enabled: polygon
            .and_then(|polygon| polygon.child_text("outline"))
            .map(|value| value != "0")
            .unwrap_or(true),
        filled: polygon
            .and_then(|polygon| polygon.child_text("fill"))
            .map(|value| value != "0")
            .unwrap_or(true),
        icon_url: icon
            .and_then(|icon| icon.child("Icon"))
            .and_then(|icon| icon.child_text("href")),
        icon_scale: icon.and_then(|icon| icon.child_f64("scale")),
        label_color_kml: label.and_then(|label| label.child_text("color")),
        highlight: None,
        balloon_text: balloon.and_then(|balloon| balloon.child_text("text")),
        balloon_bg_color_kml: balloon.and_then(|balloon| balloon.child_text("bgColor")),
        balloon_text_color_kml: balloon.and_then(|balloon| balloon.child_text("textColor")),
        balloon_display_mode: balloon
            .and_then(|balloon| balloon.child_text("displayMode"))
            .filter(|value| value == "hide"),
        list_item_type: list.and_then(|list| list.child_text("listItemType")),
        item_icon_href: list
            .and_then(|list| list.child("ItemIcon"))
            .and_then(|icon| icon.child_text("href")),
    })
}

fn parse_style_map(node: &Element, styles_by_id: &BTreeMap<String, Style>) -> Option<Style> {
    let id = node.attr("id").unwrap_or_default().to_string();
    let mut normal_ref = None;
    let mut highlight_ref = None;
    for pair in node.children_named("Pair") {
        let key = pair.child_text("key").unwrap_or_default();
        let style_url = pair
            .child_text("styleUrl")
            .map(|value| value.trim_start_matches('#').to_string());
        match key.as_str() {
            "normal" => normal_ref = style_url,
            "highlight" => highlight_ref = style_url,
            _ => {}
        }
    }
    let normal = normal_ref.and_then(|id| styles_by_id.get(&id))?;
    let highlight = highlight_ref
        .and_then(|id| styles_by_id.get(&id))
        .map(|style| StyleHighlight {
            outline_color: Some(style.outline_color.clone()),
            outline_width: Some(style.outline_width),
            fill_color: Some(style.fill_color.clone()),
            fill_opacity: Some(style.fill_opacity),
            icon_url: style.icon_url.clone(),
            icon_scale: style.icon_scale,
            label_color: style.label_color.clone(),
        });
    Some(Style {
        id,
        highlight,
        ..normal.clone()
    })
}

#[derive(Debug, Clone, Default)]
struct FeatureExtras {
    snippet: Option<String>,
    address: Option<String>,
    phone_number: Option<String>,
    time_stamp: Option<String>,
    time_span_begin: Option<String>,
    time_span_end: Option<String>,
    view: Option<PlacemarkView>,
    region: Option<PlacemarkRegion>,
}

impl From<FeatureExtras> for NewPlacemark {
    fn from(extras: FeatureExtras) -> Self {
        NewPlacemark {
            snippet: extras.snippet,
            address: extras.address,
            phone_number: extras.phone_number,
            time_stamp: extras.time_stamp,
            time_span_begin: extras.time_span_begin,
            time_span_end: extras.time_span_end,
            view: extras.view,
            region: extras.region,
            ..Default::default()
        }
    }
}

fn feature_extras(node: &Element) -> NewPlacemark {
    let time_stamp = node.child("TimeStamp").and_then(|ts| ts.child_text("when"));
    let time_span = node.child("TimeSpan");
    let view = node
        .child("Camera")
        .map(|camera| (ViewKind::Camera, camera))
        .or_else(|| {
            node.child("LookAt")
                .map(|look_at| (ViewKind::LookAt, look_at))
        })
        .map(|(kind, source)| {
            let mut params = indexmap::IndexMap::new();
            for child in &source.children {
                let text = child.text_trimmed();
                let value = match text.parse::<f64>() {
                    Ok(number) => serde_json::Value::from(number),
                    Err(_) => serde_json::Value::from(text),
                };
                params.insert(child.local_name().to_string(), value);
            }
            PlacemarkView { kind, params }
        });
    let region = node.child("Region").map(|region| {
        let mut raw = BTreeMap::new();
        region.flatten("", &mut raw);
        PlacemarkRegion { raw }
    });

    NewPlacemark {
        snippet: node.child_text("Snippet"),
        address: node.child_text("address"),
        phone_number: node.child_text("phoneNumber"),
        time_stamp,
        time_span_begin: time_span.and_then(|span| span.child_text("begin")),
        time_span_end: time_span.and_then(|span| span.child_text("end")),
        view,
        region,
        ..Default::default()
    }
}

#[derive(Debug, Clone)]
struct ParsedGeometry {
    element: KmlGeometryElement,
    rings: Vec<Vec<GeoPoint>>,
    tessellate: bool,
    extrude_height: Option<f64>,
    timestamps: Option<Vec<String>>,
}

fn parse_geometries(node: &Element) -> Vec<ParsedGeometry> {
    let mut results = Vec::new();

    for point in node.children_named("Point") {
        if let Some(coordinates) = point.child_text("coordinates") {
            let rings = stamp_altitude_mode(
                vec![parse_coordinates(&coordinates)],
                point.child_text("altitudeMode").as_deref(),
            );
            results.push(ParsedGeometry {
                element: KmlGeometryElement::Point,
                rings,
                tessellate: false,
                extrude_height: None,
                timestamps: None,
            });
        }
    }

    for polygon in node.children_named("Polygon") {
        let Some(outer) = polygon
            .child("outerBoundaryIs")
            .and_then(|outer| outer.child("LinearRing"))
            .and_then(|ring| ring.child_text("coordinates"))
            .map(|text| parse_coordinates(&text))
        else {
            continue;
        };
        let mut rings = vec![outer];
        for inner in polygon.children_named("innerBoundaryIs") {
            if let Some(ring) = inner
                .child("LinearRing")
                .and_then(|ring| ring.child_text("coordinates"))
                .map(|text| parse_coordinates(&text))
            {
                rings.push(ring);
            }
        }
        let altitude_mode = polygon.child_text("altitudeMode");
        let rings = stamp_altitude_mode(rings, altitude_mode.as_deref());
        results.push(ParsedGeometry {
            element: KmlGeometryElement::Polygon,
            tessellate: polygon
                .child_text("tessellate")
                .map(|value| value != "0")
                .unwrap_or(false),
            extrude_height: polygon
                .child_text("extrude")
                .filter(|value| value != "0")
                .map(|_| max_altitude(&rings).unwrap_or(0.0)),
            rings,
            timestamps: None,
        });
    }

    if let Some(ring) = node
        .child("LinearRing")
        .and_then(|ring| ring.child_text("coordinates"))
    {
        results.push(ParsedGeometry {
            element: KmlGeometryElement::Polygon,
            rings: vec![parse_coordinates(&ring)],
            tessellate: false,
            extrude_height: None,
            timestamps: None,
        });
    }

    for line in node.children_named("LineString") {
        if let Some(coordinates) = line.child_text("coordinates") {
            let rings = stamp_altitude_mode(
                vec![parse_coordinates(&coordinates)],
                line.child_text("altitudeMode").as_deref(),
            );
            results.push(ParsedGeometry {
                element: KmlGeometryElement::LineString,
                rings,
                tessellate: line
                    .child_text("tessellate")
                    .map(|value| value != "0")
                    .unwrap_or(false),
                extrude_height: None,
                timestamps: None,
            });
        }
    }

    if let Some(multi) = node.child("MultiGeometry") {
        results.extend(parse_geometries(multi));
    }

    for track in node.children_named("gx:Track") {
        results.push(parse_gx_track(track));
    }
    for multi_track in node.children_named("gx:MultiTrack") {
        for track in multi_track.children_named("gx:Track") {
            results.push(parse_gx_track(track));
        }
    }

    results
}

fn parse_gx_track(track: &Element) -> ParsedGeometry {
    let whens: Vec<String> = track
        .children_named("when")
        .map(|when| when.text_trimmed().to_string())
        .collect();
    let coordinates: Vec<GeoPoint> = track
        .children_named("gx:coord")
        .filter_map(|coord| parse_gx_coord(coord.text_trimmed()))
        .collect();
    let rings = stamp_altitude_mode(
        vec![coordinates],
        track.child_text("altitudeMode").as_deref(),
    );
    let coordinate_count = rings.first().map(Vec::len).unwrap_or(0);
    ParsedGeometry {
        element: KmlGeometryElement::LineString,
        rings,
        tessellate: false,
        extrude_height: None,
        timestamps: (whens.len() == coordinate_count && coordinate_count > 0).then_some(whens),
    }
}

fn parse_gx_coord(text: &str) -> Option<GeoPoint> {
    let mut parts = text.split_whitespace();
    let lon: f64 = parts.next()?.parse().ok()?;
    let lat: f64 = parts.next()?.parse().ok()?;
    let altitude: Option<f64> = parts.next().and_then(|value| value.parse().ok());
    match altitude {
        Some(altitude) => GeoPoint::with_altitude(lon, lat, altitude).ok(),
        None => GeoPoint::new(lon, lat).ok(),
    }
}

fn parse_coordinates(text: &str) -> Vec<GeoPoint> {
    text.split_whitespace()
        .filter_map(|triple| {
            let mut parts = triple.split(',');
            let lon: f64 = parts.next()?.parse().ok()?;
            let lat: f64 = parts.next()?.parse().ok()?;
            let altitude: Option<f64> = parts.next().and_then(|value| {
                let value: f64 = value.parse().ok()?;
                value.is_finite().then_some(value)
            });
            match altitude {
                Some(altitude) => GeoPoint::with_altitude(lon, lat, altitude).ok(),
                None => GeoPoint::new(lon, lat).ok(),
            }
        })
        .collect()
}

fn stamp_altitude_mode(rings: Vec<Vec<GeoPoint>>, mode: Option<&str>) -> Vec<Vec<GeoPoint>> {
    let altitude_mode = match mode {
        Some("clampToGround") => Some(terra_core::AltitudeMode::ClampToGround),
        Some("relativeToGround") => Some(terra_core::AltitudeMode::RelativeToGround),
        Some("absolute") => Some(terra_core::AltitudeMode::Absolute),
        _ => None,
    };
    match altitude_mode {
        Some(mode) => rings
            .into_iter()
            .map(|ring| {
                ring.into_iter()
                    .map(|point| GeoPoint {
                        altitude_mode: Some(mode),
                        ..point
                    })
                    .collect()
            })
            .collect(),
        None => rings,
    }
}

fn max_altitude(rings: &[Vec<GeoPoint>]) -> Option<f64> {
    rings
        .iter()
        .flatten()
        .filter_map(|point| point.altitude)
        .reduce(f64::max)
}

fn parse_extended_data(node: &Element) -> BTreeMap<String, String> {
    let mut data = BTreeMap::new();
    for extended in node.children_named("ExtendedData") {
        for entry in extended.children_named("Data") {
            let Some(name) = entry.attr("name") else {
                continue;
            };
            let value = entry.child_text("value").unwrap_or_default();
            data.insert(name.to_string(), value);
        }
        for schema in extended.children_named("SchemaData") {
            for entry in schema.children_named("SimpleData") {
                let Some(name) = entry.attr("name") else {
                    continue;
                };
                data.insert(name.to_string(), entry.text_trimmed().to_string());
            }
        }
    }
    data
}

fn non_reserved_extended_data(
    extended_data: &BTreeMap<String, String>,
) -> Option<BTreeMap<String, String>> {
    let filtered: BTreeMap<String, String> = extended_data
        .iter()
        .filter(|(key, _)| !key.starts_with(RESERVED_EXTENDED_DATA_PREFIX))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    (!filtered.is_empty()).then_some(filtered)
}

fn visibility_of(node: &Element) -> bool {
    node.child_text("visibility")
        .map(|value| value == "1")
        .unwrap_or(true)
}

fn style_id_of(node: &Element) -> Option<String> {
    node.child_text("styleUrl")
        .map(|value| value.trim_start_matches('#').to_string())
}

fn apply_meta(
    id: &mut String,
    created_at: &mut String,
    updated_at: &mut String,
    node: &Element,
    extended_data: &BTreeMap<String, String>,
) {
    if let Some(node_id) = node.attr("id") {
        *id = node_id.to_string();
    }
    if let Some(value) = extended_data.get("terra-globe:createdAt") {
        *created_at = value.clone();
    }
    if let Some(value) = extended_data.get("terra-globe:updatedAt") {
        *updated_at = value.clone();
    }
}
