//! Placemark entity, mirroring `packages/core/src/domain/placemark.ts`.

use serde::{Deserialize, Serialize};

use super::DomainError;
use super::Patch;
use super::geometry::PlacemarkGeometry;
use super::ids::new_id;
use super::time::now_iso8601;

/// Raw KML camera/look-at parameters, kept opaque for round-tripping.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlacemarkView {
    pub kind: ViewKind,
    /// Insertion order is preserved so re-export keeps the KML child order.
    pub params: indexmap::IndexMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ViewKind {
    Camera,
    LookAt,
}

/// Raw KML region (LatLonAltBox + Lod), kept opaque for round-tripping.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlacemarkRegion {
    pub raw: std::collections::BTreeMap<String, serde_json::Value>,
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
    pub order: i64,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extended_data: Option<std::collections::BTreeMap<String, String>>,
    /// Marker shared by placemarks that came from one KML MultiGeometry.
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
    pub order: Option<i64>,
    pub time_stamp: Option<String>,
    pub time_span_begin: Option<String>,
    pub time_span_end: Option<String>,
    pub snippet: Option<String>,
    pub address: Option<String>,
    pub phone_number: Option<String>,
    pub view: Option<PlacemarkView>,
    pub region: Option<PlacemarkRegion>,
    pub extended_data: Option<std::collections::BTreeMap<String, String>>,
    pub multi_geometry_group: Option<String>,
}

/// The geometry is required, so it is a separate argument rather than an
/// `Option` that could silently produce an empty placemark.
pub fn create_placemark(
    input: NewPlacemark,
    geometry: PlacemarkGeometry,
) -> Result<Placemark, DomainError> {
    if input.name.trim().is_empty() {
        return Err(DomainError::EmptyName);
    }
    let now = now_iso8601();
    Ok(Placemark {
        id: new_id(),
        folder_id: input.folder_id,
        name: input.name,
        description: input.description,
        geometry,
        style_id: input.style_id,
        visibility: input.visibility.unwrap_or(true),
        order: input.order.unwrap_or(0),
        created_at: now.clone(),
        updated_at: now,
        time_stamp: input.time_stamp,
        time_span_begin: input.time_span_begin,
        time_span_end: input.time_span_end,
        snippet: input.snippet,
        address: input.address,
        phone_number: input.phone_number,
        view: input.view,
        region: input.region,
        extended_data: input.extended_data,
        multi_geometry_group: input.multi_geometry_group,
    })
}

#[derive(Debug, Clone, Default)]
pub struct PlacemarkPatch {
    pub name: Patch<String>,
    pub folder_id: Patch<Option<String>>,
    pub description: Patch<Option<String>>,
    pub geometry: Patch<PlacemarkGeometry>,
    pub style_id: Patch<Option<String>>,
    pub visibility: Patch<bool>,
    pub order: Patch<i64>,
    pub time_stamp: Patch<Option<String>>,
    pub time_span_begin: Patch<Option<String>>,
    pub time_span_end: Patch<Option<String>>,
    pub snippet: Patch<Option<String>>,
    pub address: Patch<Option<String>>,
    pub phone_number: Patch<Option<String>>,
    pub view: Patch<Option<PlacemarkView>>,
    pub region: Patch<Option<PlacemarkRegion>>,
    pub extended_data: Patch<Option<std::collections::BTreeMap<String, String>>>,
    pub multi_geometry_group: Patch<Option<String>>,
}

impl PlacemarkPatch {
    pub fn apply(&self, placemark: &mut Placemark) {
        self.name.apply(&mut placemark.name);
        self.folder_id.apply(&mut placemark.folder_id);
        self.description.apply(&mut placemark.description);
        self.geometry.apply(&mut placemark.geometry);
        self.style_id.apply(&mut placemark.style_id);
        self.visibility.apply(&mut placemark.visibility);
        self.order.apply(&mut placemark.order);
        self.time_stamp.apply(&mut placemark.time_stamp);
        self.time_span_begin.apply(&mut placemark.time_span_begin);
        self.time_span_end.apply(&mut placemark.time_span_end);
        self.snippet.apply(&mut placemark.snippet);
        self.address.apply(&mut placemark.address);
        self.phone_number.apply(&mut placemark.phone_number);
        self.view.apply(&mut placemark.view);
        self.region.apply(&mut placemark.region);
        self.extended_data.apply(&mut placemark.extended_data);
        self.multi_geometry_group
            .apply(&mut placemark.multi_geometry_group);
    }
}
