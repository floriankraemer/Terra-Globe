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
pub mod terrain;
pub mod tiles;

pub use camera::{Camera, CameraPose, ScreenPoint};
pub use drawing::{DrawTool, DrawingSession};
pub use flat::{FlatCamera, SceneMode, lon_lat_to_plane, plane_to_lon_lat};
pub use geo::{
    Geodetic, GroundFn, MERCATOR_MAX_LAT_DEG, TileBounds, TileId, WGS84_A, WGS84_B, WGS84_E2,
    WGS84_F, ecef_to_geodetic, geo_point_to_ecef, geodetic_to_ecef, is_point_visible, no_ground,
    ray_ellipsoid_point, ray_terrain, render_height,
};
pub use globe::{Globe, GlobePick};
pub use imagery::{
    FetchedTile, ImageOverlay, ImageRgba, TileCache, TileFetcher, TileOverlay, TileProvider,
    decode_image,
};
pub use picking::{PickHit, pick};
pub use renderer::{GlobeError, GlobeRenderer, RenderStats, SURFACE_FORMAT};
pub use scene::{DistanceRings, Scene, SceneEntry, transient_style};
pub use terrain::{Terrain, TerrainCache, TerrainTile};
pub use tiles::{
    LonLatRect, SelectedTile, SelectionOptions, TileMesh, build_rect_mesh_on, build_tile_mesh,
    build_tile_mesh_on, coarser_neighbours, corner_level, select_tiles, select_tiles_with_relief,
    terrain_subdivisions,
};
