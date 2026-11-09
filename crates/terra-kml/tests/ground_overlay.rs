use terra_core::PlacemarkGeometry;
use terra_kml::{SerializeKmlInput, parse_kml, serialize_kml};

fn overlay_document(extra: &str) -> String {
    format!(
        r#"<kml><Document><GroundOverlay id="g1"><name>Map</name>{extra}<Icon><href>images/map.png</href></Icon><LatLonBox><north>10</north><south>0</south><east>20</east><west>5</west><rotation>30</rotation></LatLonBox></GroundOverlay></Document></kml>"#
    )
}

fn only_overlay(document: &str) -> (PlacemarkGeometry, Vec<String>) {
    let parsed = parse_kml(document).expect("parses");
    let placemark = parsed.placemarks.into_iter().next().expect("one overlay");
    (placemark.geometry, parsed.warnings)
}

fn opacity_of(geometry: &PlacemarkGeometry) -> Option<f64> {
    match geometry {
        PlacemarkGeometry::GroundOverlay { opacity, .. } => *opacity,
        other => panic!("not a ground overlay: {other:?}"),
    }
}

#[test]
fn color_alpha_becomes_opacity() {
    let (geometry, warnings) = only_overlay(&overlay_document("<color>80ffffff</color>"));
    assert!(warnings.is_empty());
    assert!((opacity_of(&geometry).expect("opacity") - 128.0 / 255.0).abs() < 1e-9);
}

#[test]
fn missing_color_means_no_opacity() {
    let (geometry, _) = only_overlay(&overlay_document(""));
    assert_eq!(opacity_of(&geometry), None);
}

#[test]
fn malformed_color_is_ignored_with_a_warning() {
    for bad in ["zz", "80ff", "80fffffff", "gggggggg", ""] {
        let (geometry, warnings) =
            only_overlay(&overlay_document(&format!("<color>{bad}</color>")));
        assert_eq!(opacity_of(&geometry), None, "{bad:?}");
        if !bad.is_empty() {
            assert_eq!(warnings.len(), 1, "{bad:?}");
        }
    }
}

#[test]
fn opacity_never_leaves_the_unit_range() {
    for color in ["00ffffff", "ffffffff"] {
        let (geometry, _) = only_overlay(&overlay_document(&format!("<color>{color}</color>")));
        let opacity = opacity_of(&geometry).expect("opacity");
        assert!((0.0..=1.0).contains(&opacity));
    }
}

#[test]
fn href_rotation_and_opacity_round_trip() {
    let parsed = parse_kml(&overlay_document("<color>80ffffff</color>")).expect("parses");
    let kml = serialize_kml(&SerializeKmlInput {
        folders: &parsed.folders,
        placemarks: &parsed.placemarks,
        styles: &parsed.styles,
        screen_overlays: &parsed.screen_overlays,
    })
    .expect("serializes");
    assert!(kml.contains("<color>80ffffff</color>"));
    let (geometry, warnings) = only_overlay(&kml);
    assert!(warnings.is_empty());
    match geometry {
        PlacemarkGeometry::GroundOverlay {
            bounds,
            image_url,
            rotation,
            opacity,
        } => {
            assert_eq!(image_url, "images/map.png");
            assert_eq!(rotation, Some(30.0));
            assert_eq!((bounds.north, bounds.west), (10.0, 5.0));
            assert!((opacity.expect("opacity") - 128.0 / 255.0).abs() < 1e-9);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn overlay_without_opacity_writes_no_color() {
    let parsed = parse_kml(&overlay_document("")).expect("parses");
    let kml = serialize_kml(&SerializeKmlInput {
        folders: &parsed.folders,
        placemarks: &parsed.placemarks,
        styles: &parsed.styles,
        screen_overlays: &parsed.screen_overlays,
    })
    .expect("serializes");
    assert!(!kml.contains("<color>"));
}
