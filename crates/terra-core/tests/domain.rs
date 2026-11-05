use serde_json::json;
use terra_core::domain::folder::{FolderPatch, create_folder};
use terra_core::domain::geometry::{GeoPoint, PlacemarkGeometry, RectangleBounds};
use terra_core::{DomainError, NewFolder, Patch, create_folder as mk_folder, create_placemark};

fn berlin() -> GeoPoint {
    GeoPoint::new(13.405, 52.52).expect("valid point")
}

#[test]
fn point_geometry_serializes_exactly_like_the_web_app() {
    let geometry = PlacemarkGeometry::point(berlin()).expect("valid");
    assert_eq!(
        serde_json::to_value(&geometry).expect("serializable"),
        json!({"type": "Point", "coordinates": {"lon": 13.405, "lat": 52.52}})
    );
}

#[test]
fn circle_geometry_uses_camel_case_fields() {
    let geometry = PlacemarkGeometry::circle(berlin(), 1200.0).expect("valid");
    assert_eq!(
        serde_json::to_value(&geometry).expect("serializable"),
        json!({
            "type": "Circle",
            "center": {"lon": 13.405, "lat": 52.52},
            "radiusMeters": 1200.0
        })
    );
}

#[test]
fn placemark_json_omits_absent_optionals() {
    let placemark = create_placemark(
        terra_core::NewPlacemark {
            name: "Berlin".to_string(),
            folder_id: Some("f1".to_string()),
            ..Default::default()
        },
        PlacemarkGeometry::point(berlin()).expect("valid"),
    )
    .expect("valid placemark");
    let value = serde_json::to_value(&placemark).expect("serializable");
    let object = value.as_object().expect("object");
    assert!(object.contains_key("styleId"));
    assert!(!object.contains_key("description"));
    assert_eq!(value["folderId"], json!("f1"));
    assert_eq!(value["visibility"], json!(true));
    assert_eq!(value["order"], json!(0));
}

#[test]
fn invalid_inputs_match_the_typescript_error_messages() {
    let error = GeoPoint::new(190.0, 0.0).expect_err("out of range");
    assert_eq!(
        error.to_string(),
        "longitude must be in range [-180, 180], got 190"
    );

    let error = PlacemarkGeometry::circle(berlin(), 0.0).expect_err("bad radius");
    assert_eq!(error, DomainError::InvalidRadius(0.0));

    let error = RectangleBounds::new(10.0, 20.0, 1.0, 0.0).expect_err("inverted");
    assert_eq!(error.to_string(), "south (20) must be less than north (10)");
}

#[test]
fn rectangle_validation_rejects_degenerate_bounds() {
    let error = RectangleBounds::new(10.0, 10.0, 5.0, 5.0).expect_err("degenerate");
    assert_eq!(error.to_string(), "south (10) must be less than north (10)");
}

#[test]
fn line_string_requires_two_points_and_matching_timestamps() {
    let one = vec![berlin()];
    let error = PlacemarkGeometry::line_string(one, None, None).expect_err("too short");
    assert_eq!(error.to_string(), "path must have at least 2 points, got 1");

    let path = vec![berlin(), GeoPoint::new(13.5, 52.6).expect("valid")];
    let error =
        PlacemarkGeometry::line_string(path, None, Some(vec!["2024-01-01T00:00:00Z".to_string()]))
            .expect_err("mismatch");
    assert_eq!(
        error.to_string(),
        "timestamps length (1) must match path length (2)"
    );
}

#[test]
fn folder_visibility_is_inherited() {
    let root = create_folder(NewFolder {
        name: "Root".to_string(),
        parent_id: None,
        order: 0,
        visibility: None,
        description: None,
        open: None,
    })
    .expect("valid folder");
    let child = create_folder(NewFolder {
        name: "Child".to_string(),
        parent_id: Some(root.id.clone()),
        order: 0,
        ..Default::default()
    })
    .expect("valid folder");

    let mut folders = vec![root, child.clone()];
    assert!(terra_core::is_folder_visible(&folders, Some(&child.id)));

    folders[0].visibility = false;
    assert!(!terra_core::is_folder_visible(&folders, Some(&child.id)));
    assert!(terra_core::is_folder_visible(&folders, None));
}

#[test]
fn a_folder_cycle_cannot_hang_visibility() {
    let mut a = create_folder(NewFolder {
        name: "A".to_string(),
        parent_id: None,
        order: 0,
        ..Default::default()
    })
    .expect("valid");
    let mut b = create_folder(NewFolder {
        name: "B".to_string(),
        parent_id: Some(a.id.clone()),
        order: 0,
        ..Default::default()
    })
    .expect("valid");
    a.parent_id = Some(b.id.clone());
    b.parent_id = Some(a.id.clone());
    // The cycle must terminate; an unknown folder keeps the inherited value.
    assert!(terra_core::is_folder_visible(&[a.clone(), b], Some(&a.id)));
}

#[test]
fn patch_distinguishes_unset_from_clear() {
    let mut folder = mk_folder(NewFolder {
        name: "Root".to_string(),
        parent_id: None,
        order: 0,
        visibility: None,
        description: Some("kept".to_string()),
        open: None,
    })
    .expect("valid");

    FolderPatch::default().apply(&mut folder);
    assert_eq!(folder.description.as_deref(), Some("kept"));
    assert_eq!(folder.name, "Root");

    FolderPatch {
        name: Patch::Set("Renamed".to_string()),
        description: Patch::Set(None),
        ..Default::default()
    }
    .apply(&mut folder);
    assert_eq!(folder.name, "Renamed");
    assert_eq!(folder.description, None);
}
