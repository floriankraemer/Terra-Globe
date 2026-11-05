//! Terra Globe renderer: WGS84 globe, imagery tiles, camera and picking.
//!
//! This crate knows nothing about egui, files or SQL; it renders into an
//! offscreen texture that any UI layer can display.

pub mod camera;
pub mod drawing;
pub mod flat;
pub mod geo;
pub mod globe;
pub mod imagery;
pub mod picking;
pub mod primitives;
pub mod renderer;
pub mod scene;
pub mod tiles;

pub use camera::{Camera, CameraPose, ScreenPoint};
pub use drawing::{DrawTool, DrawingSession};
pub use flat::{FlatCamera, SceneMode, lon_lat_to_plane, plane_to_lon_lat};
pub use geo::{
    Geodetic, MERCATOR_MAX_LAT_DEG, TileBounds, TileId, WGS84_A, WGS84_B, WGS84_E2, WGS84_F,
    ecef_to_geodetic, geodetic_to_ecef, is_point_visible, ray_ellipsoid_point,
};
pub use globe::{Globe, GlobePick};
pub use imagery::{FetchedTile, TileFetcher, TileProvider};
pub use picking::{PickHit, pick};
pub use renderer::{GlobeError, GlobeRenderer, RenderStats, SURFACE_FORMAT};
pub use scene::{DistanceRings, Scene, SceneEntry, transient_style};
pub use tiles::{SelectedTile, SelectionOptions, TileMesh, build_tile_mesh, select_tiles};
