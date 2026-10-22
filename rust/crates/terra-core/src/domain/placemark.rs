use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::geometry::PlacemarkGeometry;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViewKind {
    Camera,
    LookAt,
}

/// A KML number-or-string value kept opaque for round-tripping.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RawValue {
    Number(f64),
    Text(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlacemarkView {
    pub kind: ViewKind,
    /// Raw KML view params (longitude/latitude/altitude/heading/tilt/range/roll/altitudeMode).
    pub params: BTreeMap<String, RawValue>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlacemarkRegion {
    /// Raw KML LatLonAltBox + Lod params - round-trip only, no LOD behaviour.
    pub raw: BTreeMap<String, RawValue>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Placemark {
    pub id: String,
    pub folder_id: Option<String>,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub geometry: PlacemarkGeometry,
    pub style_id: Option<String>,
    pub visibility: bool,
    pub order: f64,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_stamp: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_span_begin: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_span_end: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phone_number: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<PlacemarkView>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<PlacemarkRegion>,
    /// Arbitrary KML Data/SimpleData/SchemaData, flattened name -> value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extended_data: Option<BTreeMap<String, String>>,
    /// Shared marker for placemarks that came from one KML MultiGeometry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub multi_geometry_group: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct NewPlacemark {
    pub name: String,
    pub folder_id: Option<String>,
    pub geometry: Option<PlacemarkGeometry>,
    pub description: Option<String>,
    pub style_id: Option<String>,
    pub visibility: Option<bool>,
    pub order: Option<f64>,
    pub time_stamp: Option<String>,
    pub time_span_begin: Option<String>,
    pub time_span_end: Option<String>,
    pub snippet: Option<String>,
    pub address: Option<String>,
    pub phone_number: Option<String>,
    pub view: Option<PlacemarkView>,
    pub region: Option<PlacemarkRegion>,
    pub extended_data: Option<BTreeMap<String, String>>,
    pub multi_geometry_group: Option<String>,
}

impl NewPlacemark {
    pub fn new(
        name: impl Into<String>,
        folder_id: Option<String>,
        geometry: PlacemarkGeometry,
    ) -> Self {
        Self {
            name: name.into(),
            folder_id,
            geometry: Some(geometry),
            ..Self::default()
        }
    }
}

/// Partial update (`Partial<Omit<Placemark, "id">>`). `Some(x)` sets a field;
/// nullable fields use `Some(None)` to clear.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PlacemarkPatch {
    pub folder_id: Option<Option<String>>,
    pub name: Option<String>,
    pub description: Option<Option<String>>,
    pub geometry: Option<PlacemarkGeometry>,
    pub style_id: Option<Option<String>>,
    pub visibility: Option<bool>,
    pub order: Option<f64>,
    pub updated_at: Option<String>,
    pub time_stamp: Option<Option<String>>,
    pub time_span_begin: Option<Option<String>>,
    pub time_span_end: Option<Option<String>>,
    pub snippet: Option<Option<String>>,
    pub address: Option<Option<String>>,
    pub phone_number: Option<Option<String>>,
    pub view: Option<Option<PlacemarkView>>,
    pub region: Option<Option<PlacemarkRegion>>,
    pub extended_data: Option<Option<BTreeMap<String, String>>>,
    pub multi_geometry_group: Option<Option<String>>,
}

impl Placemark {
    pub fn apply_patch(&mut self, patch: PlacemarkPatch) {
        macro_rules! set {
            ($($field:ident),*) => { $( if let Some(v) = patch.$field { self.$field = v; } )* };
        }
        set!(
            folder_id,
            name,
            description,
            geometry,
            style_id,
            visibility,
            order,
            updated_at,
            time_stamp,
            time_span_begin,
            time_span_end,
            snippet,
            address,
            phone_number,
            view,
            region,
            extended_data,
            multi_geometry_group
        );
    }
}
