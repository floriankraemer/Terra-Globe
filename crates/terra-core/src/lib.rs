//! Terra Globe domain model.
//!
//! This crate is deliberately dependency-light: no GPU, no filesystem, no OS
//! APIs. It must compile for `wasm32-unknown-unknown` with no `cfg` splits.

pub mod domain;
pub mod settings;
pub mod units;

pub use domain::circle::circle_to_polygon_ring;
pub use domain::folder::{Folder, NewFolder, create_folder};
pub use domain::geometry::{AltitudeMode, GeoPoint, PlacemarkGeometry, RectangleBounds};
pub use domain::ids::new_id;
pub use domain::measurements::{
    Measurements, geometry_center, geometry_extent_meters, haversine_meters, measurements,
    path_length_meters,
};
pub use domain::placemark::{
    NewPlacemark, Placemark, PlacemarkPatch, PlacemarkRegion, PlacemarkView, ViewKind,
    create_placemark,
};
pub use domain::route_estimate::{
    EstimatedTravelMode, StraightLineRoute, estimate_straight_line_route,
};
pub use domain::screen_overlay::{
    AnchorUnits, NewScreenOverlay, ScreenOverlay, ScreenOverlayAnchor, create_screen_overlay,
};
pub use domain::style::{
    DEFAULT_MARKER_ICON, MARKER_ICON_IDS, MarkerIconId, NewStyle, Style, StyleHighlight,
    create_style,
};
pub use domain::time::now_iso8601;
pub use domain::translate::translate_geometry;
pub use domain::visibility::{is_folder_visible, is_placemark_visible};
pub use domain::{DomainError, Patch};
pub use settings::{DEFAULT_LANGUAGE, OverlayLayerSetting, Settings};
pub use units::{
    CoordinateFormat, UnitSystem, distance_unit_label, format_area, format_coordinate,
    format_distance, format_duration, meters_to_unit, unit_to_meters,
};
