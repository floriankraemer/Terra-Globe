//! KML writer, ported from `packages/core/src/kml/serializeKml.ts`.
//!
//! Output is human-readable and stable so files diff cleanly and so the
//! round-trip tests can compare against fixtures produced by the web app.

use terra_core::domain::folder::Folder;
use terra_core::domain::geometry::{AltitudeMode, GeoPoint, PlacemarkGeometry};
use terra_core::domain::placemark::{Placemark, PlacemarkRegion, PlacemarkView, ViewKind};
use terra_core::domain::screen_overlay::{ScreenOverlay, ScreenOverlayAnchor};
use terra_core::domain::style::Style;

use crate::KmlError;
use crate::mapping::geometry::domain_geometry_to_kml;
use crate::mapping::style::{KmlStyleData, style_to_kml_data};

pub struct SerializeKmlInput<'a> {
    pub folders: &'a [Folder],
    pub placemarks: &'a [Placemark],
    pub styles: &'a [Style],
    pub screen_overlays: &'a [ScreenOverlay],
}

pub fn escape_xml(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

pub fn serialize_kml(input: &SerializeKmlInput<'_>) -> Result<String, KmlError> {
    let root_folders: Vec<&Folder> = input
        .folders
        .iter()
        .filter(|folder| folder.parent_id.is_none())
        .collect();
    let root_placemarks: Vec<&Placemark> = input
        .placemarks
        .iter()
        .filter(|placemark| placemark.folder_id.is_none())
        .collect();
    let root_overlays: Vec<&ScreenOverlay> = input
        .screen_overlays
        .iter()
        .filter(|overlay| overlay.folder_id.is_none())
        .collect();

    let mut body = String::new();
    for style in input.styles {
        body.push_str(&render_style(style)?);
    }
    for folder in root_folders {
        body.push_str(&render_folder(
            folder,
            input.folders,
            input.placemarks,
            input.screen_overlays,
        )?);
    }
    for item in group_for_render(&root_placemarks) {
        body.push_str(&render_feature_or_group(&item)?);
    }
    for overlay in root_overlays {
        body.push_str(&render_screen_overlay(overlay));
    }

    Ok(format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><kml xmlns=\"http://www.opengis.net/kml/2.2\" xmlns:gx=\"http://www.google.com/kml/ext/2.2\"><Document>{body}</Document></kml>"
    ))
}

fn render_anchor(tag: &str, anchor: &ScreenOverlayAnchor) -> String {
    format!(
        "<{tag} x=\"{}\" y=\"{}\" xunits=\"{}\" yunits=\"{}\"/>",
        format_number(anchor.x),
        format_number(anchor.y),
        anchor_units(anchor.x_units),
        anchor_units(anchor.y_units)
    )
}

fn anchor_units(units: terra_core::AnchorUnits) -> &'static str {
    match units {
        terra_core::AnchorUnits::Fraction => "fraction",
        terra_core::AnchorUnits::Pixels => "pixels",
        terra_core::AnchorUnits::InsetPixels => "insetPixels",
    }
}

fn render_screen_overlay(overlay: &ScreenOverlay) -> String {
    let size = if overlay.size_x.is_some() || overlay.size_y.is_some() {
        format!(
            "<size x=\"{}\" y=\"{}\" xunits=\"pixels\" yunits=\"pixels\"/>",
            format_number(overlay.size_x.unwrap_or(-1.0)),
            format_number(overlay.size_y.unwrap_or(-1.0))
        )
    } else {
        String::new()
    };
    let rotation = overlay
        .rotation
        .map(|rotation| format!("<rotation>{}</rotation>", format_number(rotation)))
        .unwrap_or_default();
    format!(
        "<ScreenOverlay id=\"{}\"><name>{}</name><visibility>{}</visibility><Icon><href>{}</href></Icon>{}{}{}{}</ScreenOverlay>",
        escape_xml(&overlay.id),
        escape_xml(&overlay.name),
        u8::from(overlay.visibility),
        escape_xml(&overlay.image_url),
        render_anchor("overlayXY", &overlay.overlay_xy),
        render_anchor("screenXY", &overlay.screen_xy),
        size,
        rotation
    )
}

fn altitude_mode_tag(mode: Option<AltitudeMode>) -> String {
    match mode {
        Some(mode) => format!(
            "<altitudeMode>{}</altitudeMode>",
            match mode {
                AltitudeMode::ClampToGround => "clampToGround",
                AltitudeMode::RelativeToGround => "relativeToGround",
                AltitudeMode::Absolute => "absolute",
            }
        ),
        None => String::new(),
    }
}

fn render_view(view: Option<&PlacemarkView>) -> String {
    let Some(view) = view else {
        return String::new();
    };
    let tag = match view.kind {
        ViewKind::Camera => "Camera",
        ViewKind::LookAt => "LookAt",
    };
    let mut body = String::new();
    for (key, value) in &view.params {
        body.push_str(&format!("<{key}>{}</{key}>", json_number(value)));
    }
    format!("<{tag}>{body}</{tag}>")
}

fn render_region(region: Option<&PlacemarkRegion>) -> String {
    let Some(region) = region else {
        return String::new();
    };
    let box_values = &region.raw;
    let get = |key: &str| box_values.get(key);
    let has_box = ["north", "south", "east", "west"]
        .iter()
        .any(|key| get(&format!("LatLonAltBox.{key}")).is_some());
    let has_lod = get("Lod.minLodPixels").is_some() || get("Lod.maxLodPixels").is_some();

    let box_xml = if has_box {
        let mut body = String::new();
        for key in ["north", "south", "east", "west"] {
            if let Some(value) = get(&format!("LatLonAltBox.{key}")) {
                body.push_str(&format!("<{key}>{}</{key}>", plain(value)));
            }
        }
        format!("<LatLonAltBox>{body}</LatLonAltBox>")
    } else {
        String::new()
    };
    let lod_xml = if has_lod {
        let mut body = String::new();
        for key in ["minLodPixels", "maxLodPixels"] {
            if let Some(value) = get(&format!("Lod.{key}")) {
                body.push_str(&format!("<{key}>{}</{key}>", plain(value)));
            }
        }
        format!("<Lod>{body}</Lod>")
    } else {
        String::new()
    };
    format!("<Region>{box_xml}{lod_xml}</Region>")
}

fn render_time_primitive(placemark: &Placemark) -> String {
    if let Some(time_stamp) = &placemark.time_stamp {
        return format!(
            "<TimeStamp><when>{}</when></TimeStamp>",
            escape_xml(time_stamp)
        );
    }
    if placemark.time_span_begin.is_some() || placemark.time_span_end.is_some() {
        let begin = placemark
            .time_span_begin
            .as_deref()
            .map(|value| format!("<begin>{}</begin>", escape_xml(value)))
            .unwrap_or_default();
        let end = placemark
            .time_span_end
            .as_deref()
            .map(|value| format!("<end>{}</end>", escape_xml(value)))
            .unwrap_or_default();
        return format!("<TimeSpan>{begin}{end}</TimeSpan>");
    }
    String::new()
}

fn render_feature_extras(placemark: &Placemark) -> String {
    let snippet = placemark
        .snippet
        .as_deref()
        .map(|value| format!("<Snippet>{}</Snippet>", escape_xml(value)))
        .unwrap_or_default();
    let address = placemark
        .address
        .as_deref()
        .map(|value| format!("<address>{}</address>", escape_xml(value)))
        .unwrap_or_default();
    let phone = placemark
        .phone_number
        .as_deref()
        .map(|value| format!("<phoneNumber>{}</phoneNumber>", escape_xml(value)))
        .unwrap_or_default();
    format!(
        "{snippet}{address}{phone}{}{}{}",
        render_time_primitive(placemark),
        render_view(placemark.view.as_ref()),
        render_region(placemark.region.as_ref())
    )
}

fn render_model(geometry: &PlacemarkGeometry) -> Result<String, KmlError> {
    let PlacemarkGeometry::Model {
        position,
        model_uri,
        scale,
        heading,
        tilt,
        roll,
    } = geometry
    else {
        return Err(KmlError::InvalidInput(
            "render_model called with a non-Model geometry".to_string(),
        ));
    };

    let altitude = position
        .altitude
        .map(|altitude| format!("<altitude>{}</altitude>", format_number(altitude)))
        .unwrap_or_default();
    let location = format!(
        "<Location><longitude>{}</longitude><latitude>{}</latitude>{altitude}</Location>",
        format_number(position.lon),
        format_number(position.lat)
    );
    let orientation = if heading.is_some() || tilt.is_some() || roll.is_some() {
        format!(
            "<Orientation><heading>{}</heading><tilt>{}</tilt><roll>{}</roll></Orientation>",
            format_number(heading.unwrap_or(0.0)),
            format_number(tilt.unwrap_or(0.0)),
            format_number(roll.unwrap_or(0.0))
        )
    } else {
        String::new()
    };
    let scale_xml = scale
        .map(|value| {
            format!(
                "<Scale><x>{}</x><y>{}</y><z>{}</z></Scale>",
                format_number(value),
                format_number(value),
                format_number(value)
            )
        })
        .unwrap_or_default();
    Ok(format!(
        "<Model>{location}{orientation}{scale_xml}<Link><href>{}</href></Link></Model>",
        escape_xml(model_uri)
    ))
}

fn render_ground_overlay(placemark: &Placemark) -> Result<String, KmlError> {
    let PlacemarkGeometry::GroundOverlay {
        bounds,
        image_url,
        rotation,
    } = &placemark.geometry
    else {
        return Err(KmlError::InvalidInput(
            "render_ground_overlay called with a non-GroundOverlay geometry".to_string(),
        ));
    };
    let description = placemark
        .description
        .as_deref()
        .map(|value| format!("<description>{}</description>", escape_xml(value)))
        .unwrap_or_default();
    let rotation = rotation
        .map(|rotation| format!("<rotation>{}</rotation>", format_number(rotation)))
        .unwrap_or_default();
    let bounds_xml = format!(
        "<LatLonBox><north>{}</north><south>{}</south><east>{}</east><west>{}</west>{rotation}</LatLonBox>",
        format_number(bounds.north),
        format_number(bounds.south),
        format_number(bounds.east),
        format_number(bounds.west)
    );
    let mut extended_data: Vec<(String, String)> = placemark
        .extended_data
        .iter()
        .flat_map(|map| map.iter())
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect();
    extended_data.push((
        "terra-globe:createdAt".to_string(),
        placemark.created_at.clone(),
    ));
    extended_data.push((
        "terra-globe:updatedAt".to_string(),
        placemark.updated_at.clone(),
    ));

    Ok(format!(
        "<GroundOverlay id=\"{}\"><name>{}</name>{description}{}<visibility>{}</visibility><Icon><href>{}</href></Icon>{bounds_xml}{}</GroundOverlay>",
        escape_xml(&placemark.id),
        escape_xml(&placemark.name),
        render_feature_extras(placemark),
        u8::from(placemark.visibility),
        escape_xml(image_url),
        render_extended_data(&extended_data)
    ))
}

fn render_coordinates(ring: &[GeoPoint]) -> String {
    ring.iter()
        .map(|point| match point.altitude {
            Some(altitude) => format!(
                "{},{},{}",
                format_number(point.lon),
                format_number(point.lat),
                format_number(altitude)
            ),
            None => format!("{},{}", format_number(point.lon), format_number(point.lat)),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn render_extended_data(entries: &[(String, String)]) -> String {
    if entries.is_empty() {
        return String::new();
    }
    let data: String = entries
        .iter()
        .map(|(name, value)| {
            format!(
                "<Data name=\"{}\"><value>{}</value></Data>",
                escape_xml(name),
                escape_xml(value)
            )
        })
        .collect();
    format!("<ExtendedData>{data}</ExtendedData>")
}

fn render_geometry(placemark: &Placemark) -> Result<(String, Vec<(String, String)>), KmlError> {
    if matches!(placemark.geometry, PlacemarkGeometry::Model { .. }) {
        return Ok((render_model(&placemark.geometry)?, Vec::new()));
    }
    if matches!(placemark.geometry, PlacemarkGeometry::GroundOverlay { .. }) {
        return Err(KmlError::InvalidInput(
            "GroundOverlay placemarks must be rendered via render_ground_overlay".to_string(),
        ));
    }

    let mapping = domain_geometry_to_kml(&placemark.geometry)?;
    let xml = match mapping.element {
        crate::mapping::geometry::KmlGeometryElement::Point => {
            let altitude_mode = match &placemark.geometry {
                PlacemarkGeometry::Point { coordinates } => coordinates.altitude_mode,
                _ => None,
            };
            let ring = mapping.rings.first().cloned().unwrap_or_default();
            format!(
                "<Point>{}{}</Point>",
                altitude_mode_tag(altitude_mode),
                coordinates_tag(&render_coordinates(&ring))
            )
        }
        crate::mapping::geometry::KmlGeometryElement::LineString => {
            let (tessellate, altitude_mode) = match &placemark.geometry {
                PlacemarkGeometry::LineString {
                    tessellate, path, ..
                } => (
                    tessellate.unwrap_or(false),
                    path.first().and_then(|point| point.altitude_mode),
                ),
                _ => (false, None),
            };
            let ring = mapping.rings.first().cloned().unwrap_or_default();
            format!(
                "<LineString>{}{}{}</LineString>",
                if tessellate {
                    "<tessellate>1</tessellate>"
                } else {
                    ""
                },
                altitude_mode_tag(altitude_mode),
                coordinates_tag(&render_coordinates(&ring))
            )
        }
        crate::mapping::geometry::KmlGeometryElement::Polygon => {
            let (extrude, tessellate, altitude_mode) = match &placemark.geometry {
                PlacemarkGeometry::Polygon {
                    tessellate,
                    extrude_height,
                    outer_ring,
                    ..
                } => (
                    extrude_height.is_some(),
                    tessellate.unwrap_or(false),
                    outer_ring.first().and_then(|point| point.altitude_mode),
                ),
                _ => (false, false, None),
            };
            let mut rings = mapping.rings.clone().into_iter();
            let outer = rings.next().unwrap_or_default();
            let mut inner_xml = String::new();
            for ring in rings {
                inner_xml.push_str(&format!(
                    "<innerBoundaryIs><LinearRing>{}</LinearRing></innerBoundaryIs>",
                    coordinates_tag(&render_coordinates(&ring))
                ));
            }
            format!(
                "<Polygon>{}{}{}<outerBoundaryIs><LinearRing>{}</LinearRing></outerBoundaryIs>{inner_xml}</Polygon>",
                if extrude { "<extrude>1</extrude>" } else { "" },
                if tessellate {
                    "<tessellate>1</tessellate>"
                } else {
                    ""
                },
                altitude_mode_tag(altitude_mode),
                coordinates_tag(&render_coordinates(&outer))
            )
        }
    };

    Ok((xml, mapping.extended_data))
}

fn coordinates_tag(coordinates: &str) -> String {
    format!("<coordinates>{coordinates}</coordinates>")
}

fn render_style_body(data: &KmlStyleData) -> String {
    let icon_style = data
        .icon_url
        .as_deref()
        .map(|url| {
            let scale = data
                .icon_scale
                .map(|scale| format!("<scale>{}</scale>", format_number(scale)))
                .unwrap_or_default();
            format!(
                "<IconStyle>{scale}<Icon><href>{}</href></Icon></IconStyle>",
                escape_xml(url)
            )
        })
        .unwrap_or_default();
    let label_style = data
        .label_color_kml
        .as_deref()
        .map(|color| format!("<LabelStyle><color>{color}</color></LabelStyle>"))
        .unwrap_or_default();
    let balloon_style = if data.balloon_text.is_some()
        || data.balloon_bg_color_kml.is_some()
        || data.balloon_text_color_kml.is_some()
    {
        let text = data
            .balloon_text
            .as_deref()
            .map(|text| format!("<text>{}</text>", escape_xml(text)))
            .unwrap_or_default();
        let background = data
            .balloon_bg_color_kml
            .as_deref()
            .map(|color| format!("<bgColor>{color}</bgColor>"))
            .unwrap_or_default();
        let text_color = data
            .balloon_text_color_kml
            .as_deref()
            .map(|color| format!("<textColor>{color}</textColor>"))
            .unwrap_or_default();
        let display_mode = data
            .balloon_display_mode
            .as_deref()
            .filter(|mode| *mode == "hide")
            .map(|mode| format!("<displayMode>{mode}</displayMode>"))
            .unwrap_or_default();
        format!("<BalloonStyle>{text}{background}{text_color}{display_mode}</BalloonStyle>")
    } else {
        String::new()
    };
    let list_style = if data.list_item_type.is_some() || data.item_icon_href.is_some() {
        let item_type = data
            .list_item_type
            .as_deref()
            .map(|value| format!("<listItemType>{}</listItemType>", escape_xml(value)))
            .unwrap_or_default();
        let icon = data
            .item_icon_href
            .as_deref()
            .map(|href| format!("<ItemIcon><href>{}</href></ItemIcon>", escape_xml(href)))
            .unwrap_or_default();
        format!("<ListStyle>{item_type}{icon}</ListStyle>")
    } else {
        String::new()
    };

    format!(
        "<LineStyle><color>{}</color><width>{}</width></LineStyle><PolyStyle><color>{}</color><fill>{}</fill><outline>{}</outline></PolyStyle>{icon_style}{label_style}{balloon_style}{list_style}",
        data.line_color_kml,
        format_number(data.line_width),
        data.poly_color_kml,
        u8::from(data.filled),
        u8::from(data.outline_enabled)
    )
}

fn render_style(style: &Style) -> Result<String, KmlError> {
    let data = style_to_kml_data(style);
    let Some(highlight) = data.highlight.as_ref() else {
        return Ok(format!(
            "<Style id=\"{}\">{}</Style>",
            escape_xml(&data.id),
            render_style_body(&data)
        ));
    };

    let normal_id = format!("{}-normal", data.id);
    let highlight_id = format!("{}-highlight", data.id);
    let highlight_data = KmlStyleData {
        id: highlight_id.clone(),
        line_color_kml: highlight
            .line_color_kml
            .clone()
            .unwrap_or_else(|| data.line_color_kml.clone()),
        line_width: highlight.line_width.unwrap_or(data.line_width),
        poly_color_kml: highlight
            .poly_color_kml
            .clone()
            .unwrap_or_else(|| data.poly_color_kml.clone()),
        icon_url: highlight.icon_url.clone().or_else(|| data.icon_url.clone()),
        icon_scale: highlight.icon_scale.or(data.icon_scale),
        label_color_kml: highlight
            .label_color_kml
            .clone()
            .or_else(|| data.label_color_kml.clone()),
        ..data.clone()
    };
    let style_map = format!(
        "<StyleMap id=\"{}\"><Pair><key>normal</key><styleUrl>#{}</styleUrl></Pair><Pair><key>highlight</key><styleUrl>#{}</styleUrl></Pair></StyleMap>",
        escape_xml(&data.id),
        escape_xml(&normal_id),
        escape_xml(&highlight_id)
    );
    Ok(format!(
        "{style_map}<Style id=\"{}\">{}</Style><Style id=\"{}\">{}</Style>",
        escape_xml(&normal_id),
        render_style_body(&data),
        escape_xml(&highlight_id),
        render_style_body(&highlight_data)
    ))
}

fn render_placemark(placemark: &Placemark) -> Result<String, KmlError> {
    let (geometry_xml, geometry_data) = render_geometry(placemark)?;
    let mut extended_data = geometry_data;
    if let Some(map) = &placemark.extended_data {
        extended_data.extend(
            map.iter()
                .map(|(name, value)| (name.clone(), value.clone())),
        );
    }
    extended_data.push((
        "terra-globe:createdAt".to_string(),
        placemark.created_at.clone(),
    ));
    extended_data.push((
        "terra-globe:updatedAt".to_string(),
        placemark.updated_at.clone(),
    ));
    if let Some(group) = &placemark.multi_geometry_group {
        extended_data.push(("terra-globe:multiGeometryGroup".to_string(), group.clone()));
    }

    let description = placemark
        .description
        .as_deref()
        .map(|value| format!("<description>{}</description>", escape_xml(value)))
        .unwrap_or_default();
    let style_url = placemark
        .style_id
        .as_deref()
        .map(|id| format!("<styleUrl>#{}</styleUrl>", escape_xml(id)))
        .unwrap_or_default();

    Ok(format!(
        "<Placemark id=\"{}\"><name>{}</name>{description}{}<visibility>{}</visibility>{style_url}{geometry_xml}{}</Placemark>",
        escape_xml(&placemark.id),
        escape_xml(&placemark.name),
        render_feature_extras(placemark),
        u8::from(placemark.visibility),
        render_extended_data(&extended_data)
    ))
}

fn render_feature(placemark: &Placemark) -> Result<String, KmlError> {
    if matches!(placemark.geometry, PlacemarkGeometry::GroundOverlay { .. }) {
        render_ground_overlay(placemark)
    } else {
        render_placemark(placemark)
    }
}

fn render_multi_geometry_placemark(members: &[&Placemark]) -> Result<String, KmlError> {
    let Some(first) = members.first() else {
        return Err(KmlError::InvalidInput(
            "multi-geometry group has no members".to_string(),
        ));
    };
    let mut parts = Vec::new();
    for member in members {
        parts.push(render_geometry(member)?);
    }
    let geometry_xml = format!(
        "<MultiGeometry>{}</MultiGeometry>",
        parts
            .iter()
            .map(|(xml, _)| xml.as_str())
            .collect::<String>()
    );
    let mut extended_data: Vec<(String, String)> =
        parts.iter().flat_map(|(_, data)| data.clone()).collect();
    if let Some(map) = &first.extended_data {
        extended_data.extend(
            map.iter()
                .map(|(name, value)| (name.clone(), value.clone())),
        );
    }
    extended_data.push((
        "terra-globe:createdAt".to_string(),
        first.created_at.clone(),
    ));
    extended_data.push((
        "terra-globe:updatedAt".to_string(),
        first.updated_at.clone(),
    ));

    let description = first
        .description
        .as_deref()
        .map(|value| format!("<description>{}</description>", escape_xml(value)))
        .unwrap_or_default();
    let style_url = first
        .style_id
        .as_deref()
        .map(|id| format!("<styleUrl>#{}</styleUrl>", escape_xml(id)))
        .unwrap_or_default();
    let group_id = first.multi_geometry_group.as_deref().unwrap_or_default();

    Ok(format!(
        "<Placemark id=\"{}\"><name>{}</name>{description}{}<visibility>{}</visibility>{style_url}{geometry_xml}{}</Placemark>",
        escape_xml(group_id),
        escape_xml(&first.name),
        render_feature_extras(first),
        u8::from(first.visibility),
        render_extended_data(&extended_data)
    ))
}

/// Groups placemarks sharing a `multi_geometry_group` marker so they render as
/// one `<Placemark>` with a `<MultiGeometry>`, like they were imported.
fn group_for_render<'a>(placemarks: &[&'a Placemark]) -> Vec<Vec<&'a Placemark>> {
    let mut seen: Vec<&str> = Vec::new();
    let mut result: Vec<Vec<&Placemark>> = Vec::new();
    for placemark in placemarks {
        match placemark.multi_geometry_group.as_deref() {
            None => result.push(vec![placemark]),
            Some(group) => {
                if seen.contains(&group) {
                    continue;
                }
                seen.push(group);
                result.push(
                    placemarks
                        .iter()
                        .copied()
                        .filter(|candidate| {
                            candidate.multi_geometry_group.as_deref() == Some(group)
                        })
                        .collect(),
                );
            }
        }
    }
    result
}

fn render_feature_or_group(group: &[&Placemark]) -> Result<String, KmlError> {
    if group.len() == 1 && group[0].multi_geometry_group.is_none() {
        render_feature(group[0])
    } else {
        render_multi_geometry_placemark(group)
    }
}

fn render_folder(
    folder: &Folder,
    folders: &[Folder],
    placemarks: &[Placemark],
    screen_overlays: &[ScreenOverlay],
) -> Result<String, KmlError> {
    let children_folders: Vec<&Folder> = folders
        .iter()
        .filter(|candidate| candidate.parent_id.as_deref() == Some(folder.id.as_str()))
        .collect();
    let children_placemarks: Vec<&Placemark> = placemarks
        .iter()
        .filter(|candidate| candidate.folder_id.as_deref() == Some(folder.id.as_str()))
        .collect();
    let children_overlays: Vec<&ScreenOverlay> = screen_overlays
        .iter()
        .filter(|candidate| candidate.folder_id.as_deref() == Some(folder.id.as_str()))
        .collect();

    let extended_data = render_extended_data(&[
        ("terra-globe:order".to_string(), folder.order.to_string()),
        (
            "terra-globe:createdAt".to_string(),
            folder.created_at.clone(),
        ),
        (
            "terra-globe:updatedAt".to_string(),
            folder.updated_at.clone(),
        ),
    ]);

    let mut children = String::new();
    for child in children_folders {
        children.push_str(&render_folder(child, folders, placemarks, screen_overlays)?);
    }
    for group in group_for_render(&children_placemarks) {
        children.push_str(&render_feature_or_group(&group)?);
    }
    for overlay in children_overlays {
        children.push_str(&render_screen_overlay(overlay));
    }

    let description = folder
        .description
        .as_deref()
        .map(|value| format!("<description>{}</description>", escape_xml(value)))
        .unwrap_or_default();
    let open = folder
        .open
        .map(|open| format!("<open>{}</open>", u8::from(open)))
        .unwrap_or_default();

    Ok(format!(
        "<Folder id=\"{}\"><name>{}</name>{description}{open}<visibility>{}</visibility>{extended_data}{children}</Folder>",
        escape_xml(&folder.id),
        escape_xml(&folder.name),
        u8::from(folder.visibility)
    ))
}

fn plain(value: &serde_json::Value) -> String {
    json_number(value)
}

/// JSON numbers print like JavaScript's `String(number)`: integral values have
/// no `.0`, so a view written by the web app re-serialises byte for byte.
fn json_number(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Number(value) => match value.as_f64() {
            Some(float) => format_number(float),
            None => value.to_string(),
        },
        serde_json::Value::String(text) => escape_xml(text),
        other => escape_xml(&other.to_string()),
    }
}

fn format_number(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < 1e15 {
        format!("{}", value as i64)
    } else {
        value.to_string()
    }
}
