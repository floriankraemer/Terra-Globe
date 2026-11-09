//! Ellipsoid tile meshes and quadtree LOD selection.

use std::collections::{BinaryHeap, HashMap};

use glam::{DVec3, Mat4, Vec3, Vec4};

use crate::camera::Camera;
use crate::geo::{Geodetic, GroundFn, TileId, ellipsoid_normal, geodetic_to_ecef, tile_uv_rect};

pub const DEFAULT_SUBDIVISIONS: u32 = 16;
/// One texel may cover at most this many screen pixels before the tile is
/// refined further.
pub const DEFAULT_MAX_SCREEN_ERROR_PX: f64 = 1.0;
pub const DEFAULT_MAX_TILES: usize = 160;

/// A tile mesh in tile-local meters (relative to `center_ecef`), with a skirt
/// so neighbouring tiles at different LOD levels cannot show a crack.
#[derive(Debug, Clone)]
pub struct TileMesh {
    pub positions: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
    pub center_ecef: DVec3,
}

impl TileMesh {
    pub fn vertex_count(&self) -> usize {
        self.positions.len()
    }

    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }
}

/// Grid resolution of a tile mesh that is displaced by terrain. A level-N
/// tile is textured by 256 elevation samples per side, so 64 cells is one cell
/// per four samples; the finest tiles cover so few samples that fewer cells
/// already follow every one of them.
pub fn terrain_subdivisions(z: u8) -> u32 {
    match z {
        0..=15 => 64,
        16..=17 => 32,
        _ => 16,
    }
}

/// Builds a grid mesh for one tile on the ellipsoid.
pub fn build_tile_mesh(id: TileId, subdivisions: u32) -> TileMesh {
    build_tile_mesh_on(id, subdivisions, None, [0; 4], &|tile, _| tile.z)
}

/// Terrain heights for building tile meshes. Any
/// `Fn(lon_deg, lat_deg, level) -> Option<metres>` is one.
pub trait TerrainSampler {
    /// Height from elevation no deeper than `level`; `None` where nothing is
    /// loaded.
    fn height(&self, lon_deg: f64, lat_deg: f64, level: u8) -> Option<f64>;

    /// [`Self::height`] at every node of a longitude/latitude grid, row-major
    /// (one row per latitude), with 0 where nothing is loaded. A mesh asks for
    /// its whole grid at once so that a sampler can share work between nodes.
    fn heights(&self, lons_deg: &[f64], lats_deg: &[f64], level: u8, out: &mut [f64]) {
        let mut nodes = out.iter_mut();
        for lat in lats_deg {
            for (lon, node) in lons_deg.iter().zip(&mut nodes) {
                *node = self.height(*lon, *lat, level).unwrap_or(0.0);
            }
        }
    }
}

impl<F: Fn(f64, f64, u8) -> Option<f64>> TerrainSampler for F {
    fn height(&self, lon_deg: f64, lat_deg: f64, level: u8) -> Option<f64> {
        self(lon_deg, lat_deg, level)
    }
}

impl TerrainSampler for crate::terrain::Terrain {
    fn height(&self, lon_deg: f64, lat_deg: f64, level: u8) -> Option<f64> {
        self.ground_at_level(lon_deg, lat_deg, level)
    }

    fn heights(&self, lons_deg: &[f64], lats_deg: &[f64], level: u8, out: &mut [f64]) {
        self.ground_grid_at_level(lons_deg, lats_deg, level, out);
    }
}

/// Levels of detail a tile edge can be stitched across; beyond this the
/// neighbour's grid is too coarse to line up with ours and the skirt covers it.
pub const MAX_STITCH_LEVELS: u8 = 6;

/// How many levels coarser the neighbour across each edge is, as
/// `[west, east, south, north]`; 0 for a neighbour of the same or a finer
/// level, or none. `is_leaf` says whether a tile is one of the drawn tiles.
pub fn coarser_neighbours(id: TileId, is_leaf: &dyn Fn(TileId) -> bool) -> [u8; 4] {
    let count = 1i64 << id.z;
    let (x, y) = (i64::from(id.x), i64::from(id.y));
    let coarser = |nx: i64, ny: i64| -> u8 {
        // Longitude wraps; latitude ends at the Web-Mercator limit.
        if !(0..count).contains(&ny) {
            return 0;
        }
        let mut current = TileId::new(id.z, nx.rem_euclid(count) as u32, ny as u32);
        while let Some(candidate) = current {
            if is_leaf(candidate) {
                return (id.z.saturating_sub(candidate.z)).min(MAX_STITCH_LEVELS);
            }
            current = candidate.parent();
        }
        0
    };
    [
        coarser(x - 1, y),
        coarser(x + 1, y),
        coarser(x, y + 1),
        coarser(x, y - 1),
    ]
}

/// Level a tile samples one of its four corners at: `[SW, SE, NW, NE]` is
/// corner `0..4`, the grid nodes `(row, column)` = `(0, 0)`, `(0, N)`, `(N, 0)`
/// and `(N, N)`.
///
/// A corner is shared by up to four tiles of different levels, and each would
/// sample it at its own level and put the vertex at a different height. They
/// all use the coarsest level among the drawn tiles that touch the point
/// instead, so the corner is one vertex for everybody.
pub fn corner_level(id: TileId, corner: usize, is_leaf: &dyn Fn(TileId) -> bool) -> u8 {
    let around: [(i64, i64); 3] = match corner {
        0 => [(-1, 0), (-1, 1), (0, 1)],
        1 => [(1, 0), (1, 1), (0, 1)],
        2 => [(-1, 0), (-1, -1), (0, -1)],
        _ => [(1, 0), (1, -1), (0, -1)],
    };
    let count = 1i64 << id.z;
    let mut level = id.z;
    for (dx, dy) in around {
        let y = i64::from(id.y) + dy;
        if !(0..count).contains(&y) {
            continue;
        }
        let x = (i64::from(id.x) + dx).rem_euclid(count);
        let mut current = TileId::new(id.z, x as u32, y as u32);
        while let Some(candidate) = current {
            if is_leaf(candidate) {
                level = level.min(candidate.z);
                break;
            }
            current = candidate.parent();
        }
    }
    level
}

/// Grid mesh for one tile, with every vertex raised along the ellipsoid normal
/// to the sampled terrain height when a lookup is given (`None` heights stay
/// on the ellipsoid).
///
/// Neighbours of different levels have different grids, and two grids that
/// both follow the terrain disagree between each other's vertices: a crack.
/// So an edge whose neighbour is coarser (`coarser_by`, see
/// [`coarser_neighbours`]) takes its heights from the neighbour: the nodes the
/// neighbour's grid also has are sampled at the neighbour's level, as it
/// samples them, and the nodes between are interpolated linearly, which is
/// what the neighbour's mesh does there. Such an edge is the same line from
/// both sides. The skirt remains as the fallback for what stitching cannot
/// cover (stale neighbours, grids too far apart).
///
/// `corner_level` gives the level a tile samples a corner at (see
/// [`corner_level`]); the tile's own corners and the ends of a neighbour's
/// edge, which are that neighbour's corners, use it.
pub fn build_tile_mesh_on(
    id: TileId,
    subdivisions: u32,
    ground: Option<&dyn TerrainSampler>,
    coarser_by: [u8; 4],
    corner_level: &dyn Fn(TileId, usize) -> u8,
) -> TileMesh {
    let vertices = build_tile_vertices_on(id, subdivisions, ground, coarser_by, corner_level);
    TileMesh {
        positions: vertices.positions,
        uvs: vertices.uvs,
        indices: tile_mesh_indices(subdivisions),
        center_ecef: vertices.center_ecef,
    }
}

/// The vertices of a tile mesh without its triangles, which depend only on
/// the grid resolution (see [`tile_mesh_indices`]).
#[derive(Debug, Clone)]
pub struct TileVertices {
    pub positions: Vec<[f32; 3]>,
    pub uvs: Vec<[f32; 2]>,
    pub center_ecef: DVec3,
}

/// The vertices [`build_tile_mesh_on`] builds. Every tile mesh of one grid
/// resolution has the same triangles, so a renderer keeps those once and only
/// asks for the vertices when the terrain under a tile changes.
pub fn build_tile_vertices_on(
    id: TileId,
    subdivisions: u32,
    ground: Option<&dyn TerrainSampler>,
    coarser_by: [u8; 4],
    corner_level: &dyn Fn(TileId, usize) -> u8,
) -> TileVertices {
    let subdivisions = subdivisions.clamp(1, MAX_TILE_SUBDIVISIONS);
    let bounds = id.bounds();
    let center = geodetic_to_ecef(bounds.center());
    let count = subdivisions + 1;

    let vertex_count = (count * count + 4 * count) as usize;
    let mut positions = Vec::with_capacity(vertex_count);
    let mut uvs = Vec::with_capacity(vertex_count);

    let node_lon = |column: u32| {
        bounds.west + bounds.width_deg() * f64::from(column) / f64::from(subdivisions)
    };
    let node_lat =
        |row: u32| bounds.south + bounds.height_deg() * f64::from(row) / f64::from(subdivisions);
    let lons: Vec<f64> = (0..count).map(node_lon).collect();
    let lats: Vec<f64> = (0..count).map(node_lat).collect();
    let mut heights = vec![0.0f64; (count * count) as usize];
    if let Some(ground) = ground {
        let own_level = id.z.min(crate::terrain::MAX_TERRAIN_LEVEL);
        ground.heights(&lons, &lats, own_level, &mut heights);
        let cap = |level: u8| level.min(crate::terrain::MAX_TERRAIN_LEVEL);
        for (corner, (row, column)) in [
            (0, 0),
            (0, subdivisions),
            (subdivisions, 0),
            (subdivisions, subdivisions),
        ]
        .into_iter()
        .enumerate()
        {
            heights[(row * count + column) as usize] = ground
                .height(
                    node_lon(column),
                    node_lat(row),
                    cap(corner_level(id, corner)),
                )
                .unwrap_or(0.0);
        }
        // [west, east, south, north]: the node on each edge for index `k`.
        let node_of: [&dyn Fn(u32) -> (u32, u32); 4] =
            [&|k| (k, 0), &|k| (k, subdivisions), &|k| (0, k), &|k| {
                (subdivisions, k)
            }];
        // Coarsest last, so a corner shared by two stitched edges follows the
        // coarser one.
        let mut order = [0usize, 1, 2, 3];
        order.sort_by_key(|&edge| coarser_by[edge]);
        for edge in order {
            let levels = coarser_by[edge].min(MAX_STITCH_LEVELS);
            if levels == 0 || id.z < levels {
                continue;
            }
            // The neighbour's tile along this edge shares our band (rows for
            // west/east, columns for south/north), so its bounds give its
            // grid. Its vertices are evenly spaced in degrees, and its bands
            // are not ours subdivided evenly (Mercator rows are not even in
            // latitude), so they are found from its own bounds.
            let neighbour_level = id.z - levels;
            let wrap = (1i64 << id.z) - 1;
            let neighbour_id = TileId {
                z: neighbour_level,
                x: (match edge {
                    0 => (i64::from(id.x) - 1) & wrap,
                    1 => (i64::from(id.x) + 1) & wrap,
                    _ => i64::from(id.x),
                } >> levels) as u32,
                y: (match edge {
                    2 => i64::from(id.y) + 1,
                    3 => i64::from(id.y) - 1,
                    _ => i64::from(id.y),
                } >> levels) as u32,
            };
            let neighbour = neighbour_id.bounds();
            // The corners of the neighbour that the ends of this edge are:
            // [SW, SE, NW, NE] indices of the start and the end of its edge.
            let (start_corner, end_corner) = [(1, 3), (0, 2), (2, 3), (0, 1)][edge];
            let along_latitude = edge < 2;
            let (origin, extent) = if along_latitude {
                (neighbour.south, neighbour.height_deg())
            } else {
                (neighbour.west, neighbour.width_deg())
            };
            let cells = terrain_subdivisions(neighbour_level).max(1);
            let sample_level = neighbour_level.min(crate::terrain::MAX_TERRAIN_LEVEL);
            let (fixed_row, fixed_column) = node_of[edge](0);
            let neighbour_height = |vertex: u32| {
                let sample_level = match vertex {
                    0 => cap(corner_level(neighbour_id, start_corner)),
                    v if v == cells => cap(corner_level(neighbour_id, end_corner)),
                    _ => sample_level,
                };
                let along = origin + extent * f64::from(vertex) / f64::from(cells);
                let (lon, lat) = if along_latitude {
                    (node_lon(fixed_column), along)
                } else {
                    (along, node_lat(fixed_row))
                };
                ground.height(lon, lat, sample_level).unwrap_or(0.0)
            };
            let mut lattice: HashMap<u32, f64> = HashMap::new();
            for k in 0..=subdivisions {
                let (row, column) = node_of[edge](k);
                let at = if along_latitude {
                    node_lat(row)
                } else {
                    node_lon(column)
                };
                let position =
                    ((at - origin) / extent * f64::from(cells)).clamp(0.0, f64::from(cells));
                let below = (position.floor() as u32).min(cells - 1);
                let blend = position - f64::from(below);
                let low = *lattice
                    .entry(below)
                    .or_insert_with(|| neighbour_height(below));
                let high = *lattice
                    .entry(below + 1)
                    .or_insert_with(|| neighbour_height(below + 1));
                heights[(row * count + column) as usize] = low * (1.0 - blend) + high * blend;
            }
        }
    }

    // `geodetic_to_ecef` for the whole grid, with the trigonometry done once
    // per row and per column instead of once per vertex.
    let meridians: Vec<(f64, f64)> = lons.iter().map(|lon| lon.to_radians().sin_cos()).collect();
    let (mut lowest, mut highest) = (0.0f64, 0.0f64);
    for (row, lat) in lats.iter().enumerate() {
        let (sin_lat, cos_lat) = lat.to_radians().sin_cos();
        let prime_vertical =
            crate::geo::WGS84_A / (1.0 - crate::geo::WGS84_E2 * sin_lat * sin_lat).sqrt();
        let v = 1.0 - row as f32 / subdivisions as f32;
        for (column, (sin_lon, cos_lon)) in meridians.iter().enumerate() {
            let height = heights[row * count as usize + column];
            lowest = lowest.min(height);
            highest = highest.max(height);
            let ecef = DVec3::new(
                (prime_vertical + height) * cos_lat * cos_lon,
                (prime_vertical + height) * cos_lat * sin_lon,
                (prime_vertical * (1.0 - crate::geo::WGS84_E2) + height) * sin_lat,
            );
            positions.push((ecef - center).as_vec3().to_array());
            uvs.push([column as f32 / subdivisions as f32, v]);
        }
    }

    append_skirt_vertices(
        &mut positions,
        &mut uvs,
        id,
        subdivisions,
        center,
        (highest - lowest) * 0.5,
    );
    TileVertices {
        positions,
        uvs,
        center_ecef: center,
    }
}

/// Finest grid a tile mesh is built on.
const MAX_TILE_SUBDIVISIONS: u32 = 128;

/// The triangles of every tile mesh with `subdivisions` cells per side: the
/// grid, then the four skirts.
pub fn tile_mesh_indices(subdivisions: u32) -> Vec<u32> {
    let subdivisions = subdivisions.clamp(1, MAX_TILE_SUBDIVISIONS);
    let count = subdivisions + 1;
    let mut indices = Vec::with_capacity(((subdivisions + 4) * subdivisions * 6) as usize);
    for row in 0..subdivisions {
        for column in 0..subdivisions {
            let top_left = row * count + column;
            let top_right = top_left + 1;
            let bottom_left = top_left + count;
            let bottom_right = bottom_left + 1;
            indices.extend_from_slice(&[
                top_left,
                bottom_left,
                top_right,
                top_right,
                bottom_left,
                bottom_right,
            ]);
        }
    }
    for (edge, vertices) in skirt_edges(subdivisions).into_iter().enumerate() {
        let base = count * count + edge as u32 * count;
        for pair in 0..subdivisions {
            let top_a = vertices(pair);
            let top_b = vertices(pair + 1);
            let bottom_a = base + pair;
            let bottom_b = base + pair + 1;
            indices.extend_from_slice(&[top_a, bottom_a, top_b, top_b, bottom_a, bottom_b]);
        }
    }
    indices
}

/// Longest edge of an image-overlay grid cell. Matches the vector fills, so the
/// chord sag stays well under the surface lift.
const RECT_EDGE_METERS: f64 = 5_000.0;
/// Upper bound per side; a continent-sized image would otherwise ask for
/// thousands of cells for no visible gain.
const MAX_RECT_SUBDIVISIONS: u32 = 128;
/// Minimum height of an image overlay above the ellipsoid. Images do not write
/// depth, so placemark fills draw over them whatever the lifts are.
const IMAGE_LIFT_METERS: f64 = 1.0;
/// Margin on the computed sag of the longest triangle edge, as for vector fills.
const RESIDUAL_SAG_SAFETY: f64 = 1.25;
/// Same idea on the flat plane: tiles are at 0, fills at `SURFACE_Z`.
const FLAT_IMAGE_Z: f32 = 10.0;

/// A lon/lat box in degrees. `east < west` means it crosses the antimeridian.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LonLatRect {
    pub west: f64,
    pub south: f64,
    pub east: f64,
    pub north: f64,
}

impl LonLatRect {
    /// `None` for boxes with no area, non-finite numbers or latitudes off the
    /// globe, so a broken KML cannot produce a garbage mesh.
    pub fn new(west: f64, south: f64, east: f64, north: f64) -> Option<Self> {
        let finite = [west, south, east, north]
            .iter()
            .all(|value| value.is_finite());
        let latitudes = (-90.0..=90.0).contains(&south) && (-90.0..=90.0).contains(&north);
        let rect = Self {
            west,
            south,
            east,
            north,
        };
        (finite
            && latitudes
            && north > south
            && rect.width_deg() > 0.0
            && rect.width_deg() <= 360.0)
            .then_some(rect)
    }

    pub fn crosses_antimeridian(&self) -> bool {
        self.east < self.west
    }

    pub fn width_deg(&self) -> f64 {
        if self.crosses_antimeridian() {
            self.east + 360.0 - self.west
        } else {
            self.east - self.west
        }
    }

    pub fn height_deg(&self) -> f64 {
        self.north - self.south
    }

    /// Centre as (lon, lat); the longitude is unwrapped and may exceed 180.
    pub fn center(&self) -> (f64, f64) {
        (
            self.west + self.width_deg() / 2.0,
            (self.south + self.north) / 2.0,
        )
    }
}

struct RectGrid {
    /// Height above the ellipsoid that keeps the flat triangles over the curved surface.
    lift_m: f64,
    /// Row-major from the north-west corner, longitude unwrapped.
    lon_lat: Vec<(f64, f64)>,
    uvs: Vec<[f32; 2]>,
    indices: Vec<u32>,
}

/// Lays a grid over `rect` and rotates it counter-clockwise about its centre.
///
/// The rotation happens in a locally metric frame (east scaled by cos(lat)),
/// so a square image stays square on the ground at any latitude. Rotating raw
/// degrees would shear it away from the equator. UVs stay with the unrotated
/// grid, which is what turns the picture.
fn rect_grid(rect: &LonLatRect, rotation_deg: f64) -> RectGrid {
    rect_grid_with(rect, rotation_deg, RECT_EDGE_METERS)
}

fn rect_grid_with(rect: &LonLatRect, rotation_deg: f64, edge_meters: f64) -> RectGrid {
    let (center_lon, center_lat) = rect.center();
    let east_scale = center_lat.to_radians().cos().max(1e-3);
    let width_m = rect.width_deg().to_radians() * crate::geo::WGS84_A * east_scale;
    let height_m = rect.height_deg().to_radians() * crate::geo::WGS84_A;
    let cells =
        |meters: f64| ((meters / edge_meters).ceil() as u32).clamp(1, MAX_RECT_SUBDIVISIONS);
    let (columns, rows) = (cells(width_m), cells(height_m));
    // Cells are widest at the edge nearest the equator, and their diagonal is
    // the longest edge of the triangles.
    let widest = if rect.south <= 0.0 && rect.north >= 0.0 {
        1.0
    } else {
        rect.south.abs().min(rect.north.abs()).to_radians().cos()
    };
    let longest = (rect.width_deg().to_radians() * crate::geo::WGS84_A * widest
        / f64::from(columns))
    .hypot(height_m / f64::from(rows));
    let (sin, cos) = if rotation_deg.is_finite() {
        rotation_deg.to_radians().sin_cos()
    } else {
        (0.0, 1.0)
    };

    let capacity = ((columns + 1) * (rows + 1)) as usize;
    let mut lon_lat = Vec::with_capacity(capacity);
    let mut uvs = Vec::with_capacity(capacity);
    for row in 0..=rows {
        let v = f64::from(row) / f64::from(rows);
        for column in 0..=columns {
            let u = f64::from(column) / f64::from(columns);
            let lon = rect.west + rect.width_deg() * u;
            let lat = rect.north - rect.height_deg() * v;
            let (x, y) = ((lon - center_lon) * east_scale, lat - center_lat);
            let (x, y) = (x * cos - y * sin, x * sin + y * cos);
            lon_lat.push((center_lon + x / east_scale, center_lat + y));
            uvs.push([u as f32, v as f32]);
        }
    }

    let stride = columns + 1;
    let mut indices = Vec::with_capacity((columns * rows * 6) as usize);
    for row in 0..rows {
        for column in 0..columns {
            let top_left = row * stride + column;
            let bottom_left = top_left + stride;
            indices.extend_from_slice(&[
                top_left,
                bottom_left,
                top_left + 1,
                top_left + 1,
                bottom_left,
                bottom_left + 1,
            ]);
        }
    }
    RectGrid {
        lift_m: IMAGE_LIFT_METERS
            + RESIDUAL_SAG_SAFETY * longest * longest / (6.0 * crate::geo::WGS84_A),
        lon_lat,
        uvs,
        indices,
    }
}

/// Mesh of an image overlay on the ellipsoid, lifted a metre above it.
pub fn build_rect_mesh(rect: &LonLatRect, rotation_deg: f64) -> TileMesh {
    build_rect_mesh_on(rect, rotation_deg, None)
}

/// Longest cell edge of an image overlay draped on terrain. The per-side cell
/// cap still bounds the vertex count of a large picture.
const TERRAIN_RECT_EDGE_METERS: f64 = 250.0;
/// Extra lift over terrain, on top of the plain image lift: the draped grid
/// is finer than the tile surface it lies on, not identical to it.
const TERRAIN_IMAGE_LIFT_METERS: f64 = 3.0;

/// Image overlay mesh that follows `ground` (heights above the ellipsoid)
/// when one is given, on a finer grid than the flat case.
pub fn build_rect_mesh_on(
    rect: &LonLatRect,
    rotation_deg: f64,
    ground: Option<&GroundFn<'_>>,
) -> TileMesh {
    let grid = match ground {
        Some(_) => rect_grid_with(rect, rotation_deg, TERRAIN_RECT_EDGE_METERS),
        None => rect_grid(rect, rotation_deg),
    };
    let lift = grid.lift_m
        + if ground.is_some() {
            TERRAIN_IMAGE_LIFT_METERS
        } else {
            0.0
        };
    let (center_lon, center_lat) = rect.center();
    let center = geodetic_to_ecef(Geodetic::from_degrees(center_lon, center_lat, lift));
    let positions = grid
        .lon_lat
        .iter()
        .map(|&(lon, lat)| {
            let lat = lat.clamp(-90.0, 90.0);
            let height = ground.and_then(|ground| ground(lon, lat)).unwrap_or(0.0);
            let ecef = geodetic_to_ecef(Geodetic::from_degrees(lon, lat, lift + height));
            (ecef - center).as_vec3().to_array()
        })
        .collect();
    TileMesh {
        positions,
        uvs: grid.uvs,
        indices: grid.indices,
        center_ecef: center,
    }
}

/// Mesh of an image overlay on the flat map plane. Longitudes are not wrapped,
/// so a box across the antimeridian extends past the map edge; the renderer
/// draws a second copy shifted by one world width.
pub fn build_flat_rect_mesh(rect: &LonLatRect, rotation_deg: f64) -> TileMesh {
    let grid = rect_grid(rect, rotation_deg);
    let (center_lon, center_lat) = rect.center();
    let plane = |lon: f64, lat: f64| {
        let y = terra_core::GeoPoint::new(0.0, lat.clamp(-90.0, 90.0))
            .map_or(0.0, |point| crate::flat::lon_lat_to_plane(&point).y);
        (lon / 360.0 * crate::flat::WORLD_WIDTH_METERS, y)
    };
    let (center_x, center_y) = plane(center_lon, center_lat);
    let positions = grid
        .lon_lat
        .iter()
        .map(|&(lon, lat)| {
            let (x, y) = plane(lon, lat);
            [(x - center_x) as f32, (y - center_y) as f32, FLAT_IMAGE_Z]
        })
        .collect();
    TileMesh {
        positions,
        uvs: grid.uvs,
        indices: grid.indices,
        center_ecef: DVec3::new(center_x, center_y, 0.0),
    }
}

/// The grid vertex under skirt vertex `k` of each of the four skirts: the
/// south row, the north row, the west column and the east column.
fn skirt_edges(subdivisions: u32) -> [Box<dyn Fn(u32) -> u32>; 4] {
    let count = subdivisions + 1;
    [
        Box::new(|column| column),
        Box::new(move |column| subdivisions * count + column),
        Box::new(move |row| row * count),
        Box::new(move |row| row * count + subdivisions),
    ]
}

/// Adds the vertices of a downward skirt on all four edges. The skirt depth
/// scales with the tile's ground size, which is what makes it hide cracks at
/// coarse levels too.
fn append_skirt_vertices(
    positions: &mut Vec<[f32; 3]>,
    uvs: &mut Vec<[f32; 2]>,
    id: TileId,
    subdivisions: u32,
    center: DVec3,
    min_depth: f64,
) {
    let skirt_depth = (id.ground_size_m() * 0.02)
        .clamp(20.0, 200_000.0)
        .max(min_depth);
    for vertices in skirt_edges(subdivisions) {
        for k in 0..=subdivisions {
            let original = vertices(k) as usize;
            let local = Vec3::from_array(positions[original]);
            let world = center + local.as_dvec3();
            let normal = ellipsoid_normal(world).as_vec3();
            positions.push((local - normal * skirt_depth as f32).to_array());
            uvs.push(uvs[original]);
        }
    }
}

/// A chosen tile plus the texture (which may belong to an ancestor) and its UV
/// rectangle inside that texture.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SelectedTile {
    pub id: TileId,
    pub texture: Option<(TileId, [f32; 4])>,
}

#[derive(Debug, Clone, Copy)]
pub struct SelectionOptions {
    /// Allowed size of one texel on screen, in pixels. `1.0` means the imagery
    /// is drawn at exactly its native resolution.
    pub max_screen_error_px: f64,
    pub max_tiles: usize,
    /// Side length of a tile texture; the LOD error is a texture-space error.
    pub texture_size: u32,
}

impl Default for SelectionOptions {
    fn default() -> Self {
        Self {
            max_screen_error_px: DEFAULT_MAX_SCREEN_ERROR_PX,
            max_tiles: DEFAULT_MAX_TILES,
            texture_size: crate::imagery::TILE_TEXTURE_SIZE,
        }
    }
}

/// A tile the traversal is considering, scored by how many screen pixels one of
/// its texels would cover.
#[derive(Debug, Clone, Copy)]
struct Candidate {
    id: TileId,
    error_px: f64,
}

impl Ord for Candidate {
    /// Biggest error first, so the budget goes to the blurriest part of the
    /// screen. Ties break on the tile address to keep the result deterministic.
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.error_px
            .total_cmp(&other.error_px)
            .then_with(|| other.id.cmp(&self.id))
    }
}

impl PartialOrd for Candidate {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Candidate {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == std::cmp::Ordering::Equal
    }
}

impl Eq for Candidate {}

/// Picks the tiles to draw.
///
/// The traversal is best-first: it always refines the tile whose texels are the
/// largest on screen, and stops refining a tile once its texels are at most
/// [`SelectionOptions::max_screen_error_px`] wide, at the provider's maximum
/// level, or when the tile budget is spent. Every tile it stops at becomes a
/// leaf, so the result always covers the whole visible surface - a budget cut
/// only leaves coarser (but still complete) imagery, and because it cuts where
/// the error is lowest, that is the far side of the view where blur is least
/// visible.
///
/// The result is also smooth in level: through the middle of the view the error
/// falls off gradually, so neighbours differ by at most one level. Near the
/// limb the surface is compressed to a few pixels, and a much coarser tile can
/// sit next to a fine one there without showing a seam.
pub fn select_tiles(
    camera: &Camera,
    viewport: (u32, u32),
    max_level: u8,
    options: SelectionOptions,
    texture_ready: &dyn Fn(TileId) -> bool,
) -> Vec<SelectedTile> {
    select_tiles_with_relief(camera, viewport, max_level, options, texture_ready, &|_| {
        None
    })
}

/// [`select_tiles`] for displaced terrain: `relief` gives a tile's lowest and
/// highest height (or `None` for the plain ellipsoid). The bounding sphere and
/// the horizon test grow by it, so a mountain near the limb is not culled with
/// the sea-level surface it stands on.
pub fn select_tiles_with_relief(
    camera: &Camera,
    viewport: (u32, u32),
    max_level: u8,
    options: SelectionOptions,
    texture_ready: &dyn Fn(TileId) -> bool,
    relief: &dyn Fn(TileId) -> Option<(f64, f64)>,
) -> Vec<SelectedTile> {
    if viewport.0 == 0 || viewport.1 == 0 || options.max_tiles == 0 {
        return Vec::new();
    }
    let culler = Culler::new(camera, viewport, options.texture_size, relief);
    let mut leaves: Vec<SelectedTile> = Vec::new();
    let mut pending: BinaryHeap<Candidate> = BinaryHeap::new();
    if let Some(root) = culler.candidate(TileId::root()) {
        pending.push(root);
    }

    while let Some(candidate) = pending.pop() {
        let wants_refinement =
            candidate.error_px > options.max_screen_error_px && candidate.id.z < max_level;
        let children: Vec<Candidate> = if wants_refinement {
            candidate
                .id
                .children()
                .into_iter()
                .flatten()
                .filter_map(|child| culler.candidate(child))
                .collect()
        } else {
            Vec::new()
        };
        // Swapping one pending tile for its children must stay within budget.
        // `pending` no longer contains the tile, it was just popped off.
        let tiles_after_split = leaves.len() + pending.len() + children.len();
        if !children.is_empty() && tiles_after_split <= options.max_tiles {
            pending.extend(children);
        } else {
            leaves.push(SelectedTile {
                id: candidate.id,
                texture: nearest_ready_texture(candidate.id, texture_ready),
            });
        }
    }

    leaves.sort_by_key(|tile| tile.id);
    leaves
}

/// Frustum, horizon and screen-error maths for one view.
struct Culler<'a> {
    relief: &'a dyn Fn(TileId) -> Option<(f64, f64)>,
    eye: DVec3,
    frustum: Frustum,
    screen_height: f64,
    half_fov_tan: f64,
    texture_size: f64,
}

impl<'a> Culler<'a> {
    fn new(
        camera: &Camera,
        viewport: (u32, u32),
        texture_size: u32,
        relief: &'a dyn Fn(TileId) -> Option<(f64, f64)>,
    ) -> Self {
        let aspect = f64::from(viewport.0) / f64::from(viewport.1);
        Self {
            relief,
            eye: camera.eye_ecef(),
            frustum: Frustum::from_view_projection(camera.view_projection(aspect)),
            screen_height: f64::from(viewport.1),
            half_fov_tan: (camera.fov_y / 2.0).tan(),
            texture_size: f64::from(texture_size.max(1)),
        }
    }

    fn candidate(&self, id: TileId) -> Option<Candidate> {
        let bounds = id.bounds();
        let (lowest, highest) = (self.relief)(id).unwrap_or((0.0, 0.0));
        // Heights are drawn clamped to sea level (see `Terrain::ground`).
        let (lowest, highest) = (lowest.max(0.0), highest.max(0.0));
        let mut middle = bounds.center();
        middle.height = (lowest + highest) / 2.0;
        let center = geodetic_to_ecef(middle);
        let ground_size = bounds.ground_size_m();
        let radius = ground_size * 0.75 + (highest - lowest) / 2.0;

        if !self
            .frustum
            .contains_sphere((center - self.eye).as_vec3(), radius as f32)
        {
            return None;
        }
        // Horizon test, widened by the tile's own angular size. Testing only the
        // centre drops every tile that straddles the horizon, which leaves a
        // black notch along the limb whenever the view reaches it.
        // A summit sees further than the sea level surface does, so the
        // horizon opens by the angle a point at its height looks past.
        let summit_angle = (crate::geo::WGS84_A / (crate::geo::WGS84_A + highest)).acos();
        let tile_angle = radius / crate::geo::WGS84_A + summit_angle;
        let horizon_angle = (crate::geo::WGS84_A / self.eye.length()).acos();
        let center_angle = center
            .normalize()
            .dot(self.eye.normalize())
            .clamp(-1.0, 1.0)
            .acos();
        if center_angle > horizon_angle + tile_angle {
            return None;
        }

        // Distance to the nearest point of the tile, not to its centre: a tile
        // under the camera is much closer at its near edge, and using the centre
        // would under-refine exactly the part the user is looking at.
        let distance = ((center - self.eye).length() - radius).max(1.0);
        let texel_meters = ground_size / self.texture_size;
        let error_px = texel_meters * self.screen_height / (2.0 * distance * self.half_fov_tan);
        Some(Candidate { id, error_px })
    }
}

pub(crate) fn nearest_ready_texture(
    id: TileId,
    texture_ready: &dyn Fn(TileId) -> bool,
) -> Option<(TileId, [f32; 4])> {
    let mut current = Some(id);
    while let Some(candidate) = current {
        if texture_ready(candidate) {
            return Some((candidate, tile_uv_rect(id, candidate)));
        }
        current = candidate.parent();
    }
    None
}

/// View frustum planes for culling, derived from a view-projection matrix.
#[derive(Debug, Clone, Copy)]
pub struct Frustum {
    planes: [Vec4; 5],
}

impl Frustum {
    pub fn from_view_projection(matrix: Mat4) -> Self {
        let row0 = matrix.row(0);
        let row1 = matrix.row(1);
        let row2 = matrix.row(2);
        let row3 = matrix.row(3);
        let planes = [
            normalize_plane(row3 + row0),
            normalize_plane(row3 - row0),
            normalize_plane(row3 + row1),
            normalize_plane(row3 - row1),
            normalize_plane(row3 + row2),
        ];
        Self { planes }
    }

    /// True when the sphere is at least partially inside the frustum.
    pub fn contains_sphere(&self, center: Vec3, radius: f32) -> bool {
        self.planes.iter().all(|plane| {
            plane.x * center.x + plane.y * center.y + plane.z * center.z + plane.w >= -radius
        })
    }
}

fn normalize_plane(plane: Vec4) -> Vec4 {
    let normal = plane.truncate();
    let length = normal.length();
    if length <= f32::EPSILON {
        return plane;
    }
    plane / length
}

/// Mesh for a polar cap: the area between the Web-Mercator latitude limit and
/// the pole, which no tile can cover. Rendered with the fallback colour, which
/// matches what Cesium shows over OSM (there is no imagery there either).
pub fn build_polar_cap(north: bool, segments: u32, rings: u32) -> TileMesh {
    let segments = segments.clamp(8, 128);
    let rings = rings.clamp(1, 32);
    let (start_lat, pole_lat) = if north {
        (crate::geo::MERCATOR_MAX_LAT_DEG, 90.0)
    } else {
        (-crate::geo::MERCATOR_MAX_LAT_DEG, -90.0)
    };
    let center = geodetic_to_ecef(Geodetic::from_degrees(0.0, pole_lat, 0.0));

    let mut positions = Vec::new();
    let mut uvs = Vec::new();
    let mut indices = Vec::new();
    let per_ring = segments + 1;

    for ring in 0..=rings {
        let t = f64::from(ring) / f64::from(rings);
        let lat = start_lat + (pole_lat - start_lat) * t;
        for segment in 0..=segments {
            let lon = -180.0 + 360.0 * f64::from(segment) / f64::from(segments);
            let ecef = geodetic_to_ecef(Geodetic::from_degrees(lon, lat, 0.0));
            positions.push((ecef - center).as_vec3().to_array());
            uvs.push([0.0, 0.0]);
        }
    }

    for ring in 0..rings {
        for segment in 0..segments {
            let top_left = ring * per_ring + segment;
            let top_right = top_left + 1;
            let bottom_left = top_left + per_ring;
            let bottom_right = bottom_left + 1;
            indices.extend_from_slice(&[
                top_left,
                bottom_left,
                top_right,
                top_right,
                bottom_left,
                bottom_right,
            ]);
        }
    }

    TileMesh {
        positions,
        uvs,
        indices,
        center_ecef: center,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mesh_on(
        id: TileId,
        cells: u32,
        ground: Option<&dyn TerrainSampler>,
        stitched: [u8; 4],
    ) -> TileMesh {
        build_tile_mesh_on(id, cells, ground, stitched, &|tile, _| tile.z)
    }
    use crate::geo::{ecef_to_geodetic, ray_ellipsoid_point};

    impl RectGrid {
        fn columns(&self) -> u32 {
            self.uvs.iter().filter(|uv| uv[1] == 0.0).count() as u32 - 1
        }

        fn rows(&self) -> u32 {
            self.uvs.iter().filter(|uv| uv[0] == 0.0).count() as u32 - 1
        }
    }

    fn rect(west: f64, south: f64, east: f64, north: f64) -> LonLatRect {
        LonLatRect::new(west, south, east, north).expect("valid rect")
    }

    fn corner(grid: &RectGrid, column: u32, row: u32) -> (f64, f64) {
        grid.lon_lat[(row * (grid.columns() + 1) + column) as usize]
    }

    #[test]
    fn rect_grid_covers_the_bounds_with_full_uvs() {
        let grid = rect_grid(&rect(10.0, 40.0, 12.0, 41.0), 0.0);
        let (west, north) = corner(&grid, 0, 0);
        let (east, south) = corner(&grid, grid.columns(), grid.rows());
        assert_eq!((west, north), (10.0, 41.0));
        assert!((east - 12.0).abs() < 1e-9 && (south - 40.0).abs() < 1e-9);
        assert_eq!(grid.uvs[0], [0.0, 0.0]);
        assert_eq!(grid.uvs[grid.uvs.len() - 1], [1.0, 1.0]);
        assert_eq!(
            grid.indices.len(),
            (grid.columns() * grid.rows() * 6) as usize
        );
        assert!(
            grid.indices
                .iter()
                .all(|&i| (i as usize) < grid.lon_lat.len())
        );
    }

    #[test]
    fn rect_grid_cells_stay_near_five_kilometres() {
        let grid = rect_grid(&rect(0.0, 0.0, 1.0, 1.0), 0.0);
        let cell_m = 1.0f64.to_radians() * crate::geo::WGS84_A / f64::from(grid.rows());
        assert!(cell_m <= RECT_EDGE_METERS, "cell is {cell_m} m");
        assert!(grid.rows() > 1);
    }

    #[test]
    fn huge_rects_are_capped() {
        let grid = rect_grid(&rect(-180.0, -80.0, 180.0, 80.0), 0.0);
        assert_eq!(
            (grid.columns(), grid.rows()),
            (MAX_RECT_SUBDIVISIONS, MAX_RECT_SUBDIVISIONS)
        );
    }

    #[test]
    fn antimeridian_rect_unwraps_east() {
        let crossing = rect(170.0, -5.0, -170.0, 5.0);
        assert!(crossing.crosses_antimeridian());
        assert!((crossing.width_deg() - 20.0).abs() < 1e-12);
        assert!((crossing.center().0 - 180.0).abs() < 1e-12);
        let grid = rect_grid(&crossing, 0.0);
        let (east, _) = corner(&grid, grid.columns(), 0);
        assert!(
            (east - 190.0).abs() < 1e-9,
            "east edge unwrapped, got {east}"
        );

        let mesh = build_rect_mesh(&crossing, 0.0);
        let world = mesh.center_ecef;
        assert!(
            world.x < -6.0e6,
            "centre sits on the antimeridian, {world:?}"
        );
        let max_local = mesh
            .positions
            .iter()
            .map(|p| Vec3::from_array(*p).length())
            .fold(0.0f32, f32::max);
        assert!(
            max_local < 1.3e6,
            "no vertex jumps across the globe: {max_local}"
        );
    }

    #[test]
    fn rect_validation_rejects_degenerate_boxes() {
        assert!(LonLatRect::new(0.0, 0.0, 0.0, 1.0).is_none(), "no width");
        assert!(LonLatRect::new(0.0, 1.0, 1.0, 1.0).is_none(), "no height");
        assert!(LonLatRect::new(0.0, 2.0, 1.0, 1.0).is_none(), "inverted");
        assert!(LonLatRect::new(0.0, 0.0, f64::NAN, 1.0).is_none());
        assert!(LonLatRect::new(0.0, -91.0, 1.0, 1.0).is_none());
        assert!(LonLatRect::new(-180.0, -10.0, 180.0, 10.0).is_some());
    }

    #[test]
    fn rotating_zero_degrees_is_the_identity() {
        let plain = rect_grid(&rect(10.0, 40.0, 12.0, 41.0), 0.0);
        let same = rect_grid(&rect(10.0, 40.0, 12.0, 41.0), 360.0);
        for (a, b) in plain.lon_lat.iter().zip(&same.lon_lat) {
            assert!((a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9);
        }
        let none = rect_grid(&rect(10.0, 40.0, 12.0, 41.0), f64::NAN);
        assert_eq!(none.lon_lat, plain.lon_lat);
    }

    #[test]
    fn rotating_ninety_degrees_turns_the_box_counter_clockwise() {
        // On the equator the metric frame equals degrees, so corners are exact.
        let grid = rect_grid(&rect(-1.0, -0.5, 1.0, 0.5), 90.0);
        let near = |actual: (f64, f64), expected: (f64, f64)| {
            assert!(
                (actual.0 - expected.0).abs() < 1e-6 && (actual.1 - expected.1).abs() < 1e-6,
                "{actual:?} != {expected:?}"
            );
        };
        // North-west (-1, 0.5) -> south-west (-0.5, -1); UVs stay put.
        near(corner(&grid, 0, 0), (-0.5, -1.0));
        // North-east (1, 0.5) -> north-west-ish (-0.5, 1).
        near(corner(&grid, grid.columns(), 0), (-0.5, 1.0));
        // South-east (1, -0.5) -> north-east (0.5, 1).
        near(corner(&grid, grid.columns(), grid.rows()), (0.5, 1.0));
        assert_eq!(grid.uvs[0], [0.0, 0.0]);
    }

    #[test]
    fn rotation_keeps_the_box_square_on_the_ground_away_from_the_equator() {
        let (side_deg_lat, lat) = (0.2f64, 60.0f64);
        let center_lat = lat + side_deg_lat / 2.0;
        let side_deg_lon = side_deg_lat / center_lat.to_radians().cos();
        let grid = rect_grid(&rect(0.0, lat, side_deg_lon, lat + side_deg_lat), 30.0);
        let metric = |p: (f64, f64)| (p.0 * center_lat.to_radians().cos(), p.1);
        let a = metric(corner(&grid, 0, 0));
        let b = metric(corner(&grid, grid.columns(), 0));
        let c = metric(corner(&grid, 0, grid.rows()));
        let top = (b.0 - a.0, b.1 - a.1);
        let left = (c.0 - a.0, c.1 - a.1);
        assert!(
            (top.0 * left.0 + top.1 * left.1).abs() < 1e-6,
            "edges stay perpendicular"
        );
        assert!(((top.0.hypot(top.1)) - side_deg_lat).abs() < 1e-6);
    }

    #[test]
    fn rect_mesh_vertices_hug_the_ellipsoid() {
        let mesh = build_rect_mesh(&rect(10.0, 40.0, 10.5, 40.5), 25.0);
        for position in &mesh.positions {
            let world = mesh.center_ecef + Vec3::from_array(*position).as_dvec3();
            let height = ecef_to_geodetic(world).height;
            assert!(
                (IMAGE_LIFT_METERS..3.0).contains(&height),
                "height {height}"
            );
        }
    }

    #[test]
    fn flat_rect_mesh_lies_on_the_plane_above_tiles() {
        let mesh = build_flat_rect_mesh(&rect(0.0, 0.0, 10.0, 10.0), 0.0);
        assert!(mesh.positions.iter().all(|p| p[2] == FLAT_IMAGE_Z));
        let width = mesh.positions.iter().map(|p| p[0]).fold(f32::MIN, f32::max)
            - mesh.positions.iter().map(|p| p[0]).fold(f32::MAX, f32::min);
        let expected = 10.0 / 360.0 * crate::flat::WORLD_WIDTH_METERS;
        assert!((f64::from(width) - expected).abs() < 1.0);
    }

    #[test]
    fn flat_rect_across_the_antimeridian_is_contiguous() {
        let mesh = build_flat_rect_mesh(&rect(170.0, -5.0, -170.0, 5.0), 0.0);
        let width = mesh.positions.iter().map(|p| p[0]).fold(f32::MIN, f32::max)
            - mesh.positions.iter().map(|p| p[0]).fold(f32::MAX, f32::min);
        let expected = 20.0 / 360.0 * crate::flat::WORLD_WIDTH_METERS;
        assert!((f64::from(width) - expected).abs() < 1.0, "width {width}");
    }

    fn triangle_centroid_heights(mesh: &TileMesh) -> Vec<f64> {
        mesh.indices
            .chunks_exact(3)
            .map(|triangle| {
                let sum: DVec3 = triangle
                    .iter()
                    .map(|&i| Vec3::from_array(mesh.positions[i as usize]).as_dvec3())
                    .sum();
                ecef_to_geodetic(mesh.center_ecef + sum / 3.0).height
            })
            .collect()
    }

    #[test]
    fn large_rect_meshes_stay_above_the_ellipsoid_between_vertices() {
        for (bounds, rotation) in [
            ((-60.0, -50.0, 60.0, 50.0), 0.0),
            ((-85.0, -40.0, 85.0, 40.0), 0.0),
            ((-180.0, -80.0, 180.0, 80.0), 0.0),
            ((-60.0, -50.0, 60.0, 50.0), 37.0),
            ((170.0, -30.0, -120.0, 30.0), 0.0),
        ] {
            let mesh = build_rect_mesh(&rect(bounds.0, bounds.1, bounds.2, bounds.3), rotation);
            let lowest = triangle_centroid_heights(&mesh)
                .into_iter()
                .fold(f64::MAX, f64::min);
            assert!(
                lowest > 0.0,
                "{bounds:?} rot {rotation}: centroid at {lowest} m"
            );
        }
    }

    #[test]
    fn small_rect_meshes_keep_the_plain_lift() {
        let mesh = build_rect_mesh(&rect(10.0, 40.0, 10.001, 40.0008), 0.0);
        for height in triangle_centroid_heights(&mesh) {
            assert!(
                (height - IMAGE_LIFT_METERS).abs() < 0.01,
                "a 100 m box must not be over-lifted, got {height}"
            );
        }
    }

    fn ready(_: TileId) -> bool {
        false
    }

    #[test]
    fn tile_mesh_has_grid_and_skirt_vertices() {
        let mesh = build_tile_mesh(TileId::new(6, 33, 22).expect("valid"), 8);
        let grid = 9 * 9;
        assert_eq!(mesh.vertex_count(), grid + 4 * 9);
        assert!(mesh.triangle_count() > 8 * 8 * 2);
        // Local positions are small; the absolute ECEF lives in the centre.
        let max_local = mesh
            .positions
            .iter()
            .map(|p| Vec3::from_array(*p).length())
            .fold(0.0f32, f32::max);
        assert!(max_local < 5.0e5, "local vertices must stay near the tile");
        assert!(mesh.center_ecef.length() > 6.0e6);
    }

    #[test]
    fn tile_mesh_vertices_lie_on_the_ellipsoid_plus_skirt() {
        let id = TileId::new(3, 4, 2).expect("valid");
        let mesh = build_tile_mesh(id, 4);
        let grid = 5 * 5;
        for position in &mesh.positions[..grid] {
            let world = mesh.center_ecef + Vec3::from_array(*position).as_dvec3();
            let geodetic = crate::geo::ecef_to_geodetic(world);
            assert!(geodetic.height.abs() < 1.0, "height {}", geodetic.height);
        }
    }

    #[test]
    fn zoomed_out_selects_few_tiles_and_zoomed_in_selects_more() {
        let far = Camera::default();
        let far_tiles = select_tiles(&far, (1280, 800), 19, SelectionOptions::default(), &ready);

        let near = Camera {
            distance: 2_000.0,
            pitch: 60f64.to_radians(),
            ..Camera::default()
        };
        let near_tiles = select_tiles(&near, (1280, 800), 19, SelectionOptions::default(), &ready);

        assert!(!far_tiles.is_empty());
        assert!(!near_tiles.is_empty());
        assert!(
            near_tiles.iter().map(|t| t.id.z).max().unwrap_or(0)
                > far_tiles.iter().map(|t| t.id.z).max().unwrap_or(0),
            "zooming in must increase tile level"
        );
        assert!(near_tiles.len() <= DEFAULT_MAX_TILES);
        assert!(far_tiles.len() <= DEFAULT_MAX_TILES);
    }

    #[test]
    fn selection_respects_the_tile_budget() {
        let camera = Camera {
            distance: 100.0,
            ..Camera::default()
        };
        let options = SelectionOptions {
            max_tiles: 8,
            ..Default::default()
        };
        let tiles = select_tiles(&camera, (1920, 1080), 19, options, &ready);
        assert!(tiles.len() <= 8, "got {} tiles", tiles.len());
    }

    /// Surface point under a screen pixel, if the ray hits the globe.
    fn surface_at(camera: &Camera, viewport: (u32, u32), x: f64, y: f64) -> Option<(f64, f64)> {
        let (origin, direction) = camera.ray_at((x, y), viewport);
        let hit = ray_ellipsoid_point(origin, direction)?;
        let geodetic = ecef_to_geodetic(hit);
        Some((geodetic.lon_deg(), geodetic.lat_deg()))
    }

    /// Containment with a hair of tolerance: a ray that lands exactly on a tile
    /// edge can differ from the bounds by one ULP, which is a test artefact and
    /// not a hole (the neighbouring meshes share that edge exactly).
    const EDGE_EPSILON_DEG: f64 = 1e-6;

    fn contains(tile: TileId, lon: f64, lat: f64) -> bool {
        let bounds = tile.bounds();
        lon >= bounds.west - EDGE_EPSILON_DEG
            && lon < bounds.east + EDGE_EPSILON_DEG
            && lat >= bounds.south - EDGE_EPSILON_DEG
            && lat < bounds.north + EDGE_EPSILON_DEG
    }

    /// The selected tile that contains a lon/lat, if the surface point is covered.
    fn covering(tiles: &[SelectedTile], lon: f64, lat: f64) -> Option<&SelectedTile> {
        tiles.iter().find(|tile| contains(tile.id, lon, lat))
    }

    /// Screen pixels covered by one texel of `tile` at a surface point.
    fn texel_px(camera: &Camera, viewport: (u32, u32), tile: TileId, lon: f64, lat: f64) -> f64 {
        let point = geodetic_to_ecef(Geodetic::from_degrees(lon, lat, 0.0));
        let distance = (point - camera.eye_ecef()).length().max(1.0);
        let texel = tile.ground_size_m() / f64::from(SelectionOptions::default().texture_size);
        texel * f64::from(viewport.1) / (2.0 * distance * (camera.fov_y / 2.0).tan())
    }

    fn poses() -> Vec<(Camera, (u32, u32))> {
        let mut poses = Vec::new();
        for (distance, pitch_deg) in [
            (40_000_000.0, 90.0),
            (20_000_000.0, 45.0),
            (8_000_000.0, 45.0),
            (2_000_000.0, 55.0),
            (200_000.0, 60.0),
            (20_000.0, 80.0f64),
        ] {
            for heading_deg in [0.0f64, 65.0, 180.0] {
                let camera = Camera {
                    target_lon_deg: -10.0,
                    target_lat_deg: 35.0,
                    distance,
                    pitch: pitch_deg.to_radians(),
                    heading: heading_deg.to_radians(),
                    ..Camera::default()
                };
                poses.push((camera, (1280, 800)));
            }
        }
        poses
    }

    /// Every pixel that shows the globe must be covered by a tile.
    ///
    /// Regression test for the old traversal, which `break`ed as soon as the
    /// tile budget was reached: that left whole regions of the globe without any
    /// tile, so the globe rendered as a crescent of fine tiles surrounded by
    /// nothing.
    #[test]
    fn every_visible_globe_pixel_is_covered() {
        for (camera, viewport) in poses() {
            let tiles = select_tiles(&camera, viewport, 19, SelectionOptions::default(), &ready);
            let mut samples = 0;
            let mut problems = Vec::new();
            let mut y = 0.0;
            while y < f64::from(viewport.1) {
                let mut x = 0.0;
                while x < f64::from(viewport.0) {
                    if let Some((lon, lat)) = surface_at(&camera, viewport, x, y) {
                        // Beyond the Mercator latitude limit there is no tile at
                        // all; the renderer draws the polar cap meshes instead.
                        if lat.abs() > crate::geo::MERCATOR_MAX_LAT_DEG {
                            x += 16.0;
                            continue;
                        }
                        samples += 1;
                        let hits = tiles
                            .iter()
                            .filter(|tile| contains(tile.id, lon, lat))
                            .count();
                        // Two hits only happens for a sample that lands exactly on
                        // a shared tile edge, where the tolerance above counts both
                        // neighbours; a miss is the defect this test is for.
                        if hits == 0 {
                            problems.push((x, y));
                        }
                    }
                    x += 16.0;
                }
                y += 16.0;
            }
            assert!(samples > 200, "pose saw almost no globe: {samples} samples");
            assert!(
                problems.is_empty(),
                "pose distance {} pitch {} heading {}: {} of {samples} globe pixels are not covered (first: {:?}), {} tiles selected",
                camera.distance,
                camera.pitch.to_degrees(),
                camera.heading.to_degrees(),
                problems.len(),
                problems.first(),
                tiles.len()
            );
        }
    }

    /// One texel must never cover much more than one pixel of a visible globe
    /// pixel: that is exactly what "low resolution" looks like.
    #[test]
    fn visible_imagery_is_near_native_resolution() {
        for (camera, viewport) in poses() {
            let tiles = select_tiles(&camera, viewport, 19, SelectionOptions::default(), &ready);
            let mut worst = 0.0f64;
            let mut worst_at = None;
            let mut y = 0.0;
            while y < f64::from(viewport.1) {
                let mut x = 0.0;
                while x < f64::from(viewport.0) {
                    if let Some((lon, lat)) = surface_at(&camera, viewport, x, y) {
                        if lat.abs() <= crate::geo::MERCATOR_MAX_LAT_DEG
                            && let Some(tile) = covering(&tiles, lon, lat)
                        {
                            let error = texel_px(&camera, viewport, tile.id, lon, lat);
                            if error > worst {
                                worst = error;
                                worst_at = Some((x, y, tile.id, lon, lat));
                            }
                        }
                    }
                    x += 8.0;
                }
                y += 8.0;
            }
            assert!(
                worst <= 2.5,
                "pose distance {} pitch {} heading {}: {worst:.2} px per texel at {worst_at:?}",
                camera.distance,
                camera.pitch.to_degrees(),
                camera.heading.to_degrees()
            );
        }
    }

    /// The whole point of the LOD metric: at the point the user is looking at,
    /// one texel should cover about one pixel - not four (blurry) and not a
    /// quarter (wasted bandwidth).
    #[test]
    fn the_focused_tile_is_close_to_native_resolution() {
        for (camera, viewport) in poses() {
            let tiles = select_tiles(&camera, viewport, 19, SelectionOptions::default(), &ready);
            let lon = camera.target_lon_deg;
            let lat = camera.target_lat_deg;
            let tile = covering(&tiles, lon, lat).expect("the camera target must be covered");
            let error = texel_px(&camera, viewport, tile.id, lon, lat);
            assert!(
                (0.4..=1.6).contains(&error),
                "pose {} pitch {}: {:?} puts {error:.2} px on one texel at the target",
                camera.distance,
                camera.pitch.to_degrees(),
                tile.id
            );
        }
    }

    #[test]
    fn zooming_in_raises_the_level_monotonically() {
        let viewport = (1280, 800);
        let mut previous = 0;
        let mut distance = 40_000_000.0;
        while distance > 50.0 {
            let camera = Camera {
                target_lon_deg: 13.4,
                target_lat_deg: 52.5,
                distance,
                pitch: 60f64.to_radians(),
                ..Camera::default()
            };
            let tiles = select_tiles(&camera, viewport, 19, SelectionOptions::default(), &ready);
            let tile = covering(&tiles, 13.4, 52.5).expect("target covered");
            assert!(
                tile.id.z >= previous,
                "zooming from {} to {} lowered the level at the target from {previous} to {}",
                40_000_000.0,
                distance,
                tile.id.z
            );
            previous = tile.id.z;
            distance /= 2.0;
        }
        assert_eq!(
            previous, 19,
            "the deepest zoom must reach the maximum level"
        );
    }

    #[test]
    fn selection_respects_max_level() {
        let camera = Camera {
            distance: 20.0,
            ..Camera::default()
        };
        let tiles = select_tiles(&camera, (1280, 800), 4, SelectionOptions::default(), &ready);
        assert!(tiles.iter().all(|tile| tile.id.z <= 4));
    }

    #[test]
    fn texture_falls_back_to_a_ready_ancestor_with_the_right_uv_rect() {
        let camera = Camera {
            distance: 5_000.0,
            ..Camera::default()
        };
        let ready_ancestor = |id: TileId| id.z == 0 || (id.z == 1 && id.x == 1 && id.y == 0);
        let tiles = select_tiles(
            &camera,
            (1280, 800),
            19,
            SelectionOptions::default(),
            &ready_ancestor,
        );
        for tile in &tiles {
            if let Some((texture, uv)) = tile.texture {
                assert!(texture.z <= tile.id.z);
                assert_eq!(uv, tile_uv_rect(tile.id, texture));
                assert!(uv[2] > 0.0 && uv[3] > 0.0);
            }
        }
    }

    #[test]
    fn polar_caps_reach_the_poles_and_cover_the_gap() {
        for north in [true, false] {
            let cap = build_polar_cap(north, 32, 4);
            assert!(cap.triangle_count() > 0);
            let pole = if north { 90.0 } else { -90.0 };
            let geodetic = crate::geo::ecef_to_geodetic(
                cap.center_ecef
                    + Vec3::from_array(cap.positions[cap.positions.len() - 1]).as_dvec3(),
            );
            assert!((geodetic.lat_deg() - pole).abs() < 1e-6);
            // No vertex sticks out beyond the cap's own extent.
            let max = cap
                .positions
                .iter()
                .map(|p| Vec3::from_array(*p).length())
                .fold(0.0f32, f32::max);
            assert!(max < 6.0e5, "cap vertices must stay near the pole: {max}");
        }
    }

    #[test]
    fn frustum_rejects_spheres_behind_the_camera() {
        let camera = Camera::default();
        let frustum = Frustum::from_view_projection(camera.view_projection(1.6));
        let behind = (camera.eye_ecef() - camera.target_ecef())
            .as_vec3()
            .normalize()
            * 1.0e7;
        assert!(!frustum.contains_sphere(behind, 1.0));
    }

    fn height_of(mesh: &TileMesh, index: usize) -> f64 {
        let local = Vec3::from_array(mesh.positions[index]).as_dvec3();
        ecef_to_geodetic(mesh.center_ecef + local).height
    }

    #[test]
    fn terrain_raises_every_grid_vertex_along_the_normal() {
        let id = TileId::new(8, 136, 90).expect("tile");
        let plain = mesh_on(id, 8, None, [0; 4]);
        let raised = mesh_on(id, 8, Some(&|_, _, _| Some(1_250.0)), [0; 4]);
        assert_eq!(plain.vertex_count(), raised.vertex_count());
        assert_eq!(plain.indices, raised.indices);
        let grid = 9 * 9;
        for index in 0..grid {
            assert!(height_of(&plain, index).abs() < 0.5);
            assert!(
                (height_of(&raised, index) - 1_250.0).abs() < 0.5,
                "vertex {index} at {}",
                height_of(&raised, index)
            );
        }
        assert_eq!(plain.uvs, raised.uvs, "terrain never moves the texture");
    }

    #[test]
    fn unknown_ground_leaves_the_tile_on_the_ellipsoid() {
        let id = TileId::new(8, 136, 90).expect("tile");
        let missing = mesh_on(id, 8, Some(&|_, _, _| None), [0; 4]);
        let plain = build_tile_mesh(id, 8);
        assert_eq!(missing.positions, plain.positions);
        assert!(missing.positions.iter().flatten().all(|v| v.is_finite()));
    }

    #[test]
    fn skirts_grow_with_the_relief_so_lod_seams_stay_closed() {
        let id = TileId::new(14, 8_800, 5_400).expect("tile");
        let bounds = id.bounds();
        let ramp =
            move |lon: f64, _: f64, _: u8| Some((lon - bounds.west) / bounds.width_deg() * 900.0);
        let plain = mesh_on(id, 4, Some(&|_, _, _| Some(0.0)), [0; 4]);
        let steep = mesh_on(id, 4, Some(&ramp), [0; 4]);
        let grid = 5 * 5;
        // First skirt vertex hangs below the first grid vertex of the south edge.
        let drop = |mesh: &TileMesh| height_of(mesh, 0) - height_of(mesh, grid);
        assert!(
            drop(&plain) < 60.0,
            "flat tiles keep the small skirt: {}",
            drop(&plain)
        );
        assert!(
            drop(&steep) >= 449.0,
            "450 m skirt under 900 m of relief: {}",
            drop(&steep)
        );
    }

    #[test]
    fn terrain_resolution_is_finer_for_coarse_tiles() {
        assert_eq!(terrain_subdivisions(3), 64);
        assert_eq!(terrain_subdivisions(15), 64);
        assert!(terrain_subdivisions(17) < 64);
        assert!(terrain_subdivisions(19) <= terrain_subdivisions(17));
    }

    #[test]
    fn a_draped_image_follows_the_ground_on_a_finer_grid() {
        let rect = LonLatRect::new(10.0, 20.0, 10.05, 20.05).expect("rect");
        let hill = |lon: f64, _: f64| Some((lon - 10.0) * 20_000.0);
        let flat = build_rect_mesh(&rect, 0.0);
        let draped = build_rect_mesh_on(&rect, 0.0, Some(&hill));
        assert!(draped.vertex_count() > flat.vertex_count() * 4);
        let heights: Vec<f64> = (0..draped.vertex_count())
            .map(|index| height_of(&draped, index))
            .collect();
        let (low, high) = heights
            .iter()
            .fold((f64::MAX, f64::MIN), |(l, h), v| (l.min(*v), h.max(*v)));
        assert!(low < 5.0 && high > 900.0, "draped heights {low}..{high}");
        let plain_heights: Vec<f64> = (0..flat.vertex_count())
            .map(|index| height_of(&flat, index))
            .collect();
        assert!(plain_heights.iter().all(|h| *h < 5.0));
    }

    #[test]
    fn a_summit_just_past_the_horizon_is_not_culled_with_its_tile() {
        // Eye ~50 km up, west of the target, looking east along the surface; the sea level horizon
        // ends well before the tile, a 8 km summit on it is still in view.
        let camera = Camera {
            target_lon_deg: 10.0,
            target_lat_deg: 0.0,
            distance: 300_000.0,
            heading: 270f64.to_radians(),
            pitch: 10f64.to_radians(),
            ..Camera::default()
        };
        let id = TileId::new(8, 140, 127).expect("tile");
        let flat = |_: TileId| None;
        let high = |_: TileId| Some((0.0, 8_000.0));
        let without = Culler::new(&camera, (800, 600), 256, &flat).candidate(id);
        let with = Culler::new(&camera, (800, 600), 256, &high).candidate(id);
        assert!(without.is_none(), "sea level is below the horizon here");
        assert!(with.is_some(), "the summit rises into view");
    }

    #[test]
    fn tall_relief_widens_the_bounding_volume_so_tiles_are_not_cut_early() {
        let camera = Camera::default();
        let id = TileId::new(2, 2, 1).expect("tile");
        let none = |_: TileId| None;
        let tall = |_: TileId| Some((0.0, 8_000.0));
        let a = Culler::new(&camera, (800, 600), 256, &none)
            .candidate(id)
            .expect("visible");
        let b = Culler::new(&camera, (800, 600), 256, &tall)
            .candidate(id)
            .expect("visible");
        assert_eq!(a.id, b.id);
        assert!(b.error_px >= a.error_px, "a larger sphere is never farther");
    }

    /// Ridges every ~1.3 km along a meridian or parallel: any two grids that
    /// sample this at different nodes disagree between them.
    fn ridged(lon: f64, lat: f64, _level: u8) -> Option<f64> {
        Some(
            600.0
                + 400.0 * (lat * std::f64::consts::TAU / 0.012).sin()
                + 300.0 * (lon * std::f64::consts::TAU / 0.009).cos(),
        )
    }

    /// Geodetic `(lon, lat, height)` of the mesh vertex at grid node `(row, column)`.
    fn node(mesh: &TileMesh, cells: u32, row: u32, column: u32) -> (f64, f64, f64) {
        let local = Vec3::from_array(mesh.positions[(row * (cells + 1) + column) as usize]);
        let geodetic = ecef_to_geodetic(mesh.center_ecef + local.as_dvec3());
        (geodetic.lon_deg(), geodetic.lat_deg(), geodetic.height)
    }

    /// Height of `coarse`'s polyline along its east edge at `lat`.
    fn east_edge_height(coarse: &TileMesh, cells: u32, lat: f64) -> f64 {
        let edge: Vec<(f64, f64, f64)> = (0..=cells)
            .map(|row| node(coarse, cells, row, cells))
            .collect();
        // Vertex positions are f32 relative to the tile, so a latitude on the
        // very end of the edge can land a hair outside it.
        let lat = lat.clamp(edge[0].1, edge[edge.len() - 1].1);
        let pair = edge
            .windows(2)
            .find(|pair| pair[0].1 <= lat && lat <= pair[1].1)
            .expect("latitude on the coarse edge");
        let t = (lat - pair[0].1) / (pair[1].1 - pair[0].1);
        pair[0].2 * (1.0 - t) + pair[1].2 * t
    }

    #[test]
    fn a_finer_tile_follows_the_coarser_neighbours_edge_exactly() {
        for levels in 1u8..=3 {
            let coarse_id = TileId::new(8, 136, 90).expect("tile");
            // The tile east of it, `levels` levels finer, in the row band's middle.
            let fine_z = 8 + levels;
            let scale = 1u32 << levels;
            let fine_id =
                TileId::new(fine_z, 137 * scale, 90 * scale + scale / 2).expect("fine tile");
            let cells = terrain_subdivisions(8);
            let coarse = mesh_on(coarse_id, cells, Some(&ridged), [0; 4]);
            let stitched = mesh_on(
                fine_id,
                terrain_subdivisions(fine_z),
                Some(&ridged),
                [levels, 0, 0, 0],
            );
            let fine_cells = terrain_subdivisions(fine_z);
            for row in 0..=fine_cells {
                let (_, lat, height) = node(&stitched, fine_cells, row, 0);
                let expected = east_edge_height(&coarse, cells, lat);
                assert!(
                    (height - expected).abs() < 0.05,
                    "{levels} levels apart, node {row}: {height} vs {expected}"
                );
            }
        }
    }

    #[test]
    fn without_stitching_the_edges_disagree_and_the_skirt_covers_the_gap() {
        let coarse_id = TileId::new(8, 136, 90).expect("tile");
        let fine_id = TileId::new(10, 137 * 4, 90 * 4 + 2).expect("fine tile");
        let cells = terrain_subdivisions(8);
        let coarse = mesh_on(coarse_id, cells, Some(&ridged), [0; 4]);
        let unstitched = mesh_on(fine_id, terrain_subdivisions(10), Some(&ridged), [0; 4]);
        let fine_cells = terrain_subdivisions(10);
        let worst = (0..=fine_cells)
            .map(|row| {
                let (_, lat, height) = node(&unstitched, fine_cells, row, 0);
                (height - east_edge_height(&coarse, cells, lat)).abs()
            })
            .fold(0.0, f64::max);
        assert!(worst > 5.0, "the ridges must open a gap: {worst} m");

        // The skirt under every edge vertex reaches below the neighbour's edge.
        let grid = ((fine_cells + 1) * (fine_cells + 1)) as usize;
        let skirt_depth = (0..=fine_cells)
            .map(|row| {
                let top = node(&unstitched, fine_cells, row, 0).2;
                // West skirt vertices come third: after south and north.
                let skirt = Vec3::from_array(
                    unstitched.positions[grid + 2 * (fine_cells as usize + 1) + row as usize],
                );
                let below = ecef_to_geodetic(unstitched.center_ecef + skirt.as_dvec3()).height;
                top - below
            })
            .fold(f64::MAX, f64::min);
        assert!(
            skirt_depth > worst,
            "skirt {skirt_depth} m must cover the {worst} m gap"
        );
    }

    #[test]
    fn stitching_touches_only_the_edges_that_have_a_coarser_neighbour() {
        let id = TileId::new(10, 548, 362).expect("tile");
        let cells = terrain_subdivisions(10);
        let plain = mesh_on(id, cells, Some(&ridged), [0; 4]);
        let stitched = mesh_on(id, cells, Some(&ridged), [0, 1, 0, 0]);
        let mut changed_columns = std::collections::BTreeSet::new();
        for row in 0..=cells {
            for column in 0..=cells {
                let index = (row * (cells + 1) + column) as usize;
                if plain.positions[index] != stitched.positions[index] {
                    changed_columns.insert(column);
                }
            }
        }
        assert_eq!(
            changed_columns.into_iter().collect::<Vec<_>>(),
            vec![cells],
            "only the east column moves"
        );
        assert_eq!(plain.uvs, stitched.uvs, "the texture does not move");
    }

    #[test]
    fn skirts_reuse_their_edge_uvs_exactly() {
        let mesh = mesh_on(
            TileId::new(9, 270, 180).expect("tile"),
            8,
            Some(&ridged),
            [0; 4],
        );
        let grid = 9 * 9;
        // Four skirts of nine vertices follow the grid, each copying an edge.
        let edge_rows: [Vec<usize>; 4] = [
            (0..9).collect(),
            (0..9).map(|column| 8 * 9 + column).collect(),
            (0..9).map(|row| row * 9).collect(),
            (0..9).map(|row| row * 9 + 8).collect(),
        ];
        for (edge, nodes) in edge_rows.iter().enumerate() {
            for (position, node) in nodes.iter().enumerate() {
                assert_eq!(mesh.uvs[grid + edge * 9 + position], mesh.uvs[*node]);
            }
        }
    }

    #[test]
    fn neighbours_are_classified_by_how_much_coarser_they_are() {
        use std::collections::HashSet;
        let id = TileId::new(6, 24, 20).expect("tile");
        let leaves: HashSet<TileId> = [
            id,
            // West: a level 3 tile covers (16..23, 16..23).
            TileId::new(3, 2, 2).expect("tile"),
            // North: a level 4 tile covers (24..27, 18..19).
            TileId::new(4, 6, 4).expect("tile"),
            // South: finer tiles, which stitch to us, not the other way round.
            TileId::new(7, 48, 42).expect("tile"),
            TileId::new(7, 49, 42).expect("tile"),
        ]
        .into_iter()
        .collect();
        let found = coarser_neighbours(id, &|candidate| leaves.contains(&candidate));
        // [west, east, south, north]: nothing is drawn east of it.
        assert_eq!(found, [3, 0, 0, 2]);
    }

    #[test]
    fn neighbour_lookup_wraps_longitude_and_stops_at_the_limit() {
        use std::collections::HashSet;
        let first = TileId::new(4, 0, 5).expect("tile");
        let last = TileId::new(3, 7, 2).expect("tile");
        let leaves: HashSet<TileId> = [first, last].into_iter().collect();
        let found = coarser_neighbours(first, &|candidate| leaves.contains(&candidate));
        assert_eq!(found[0], 1, "west of x = 0 is the last column");
        let top = TileId::new(4, 3, 0).expect("tile");
        let found = coarser_neighbours(top, &|_| true);
        assert_eq!(found[3], 0, "nothing lies north of the top row");
        // The cap keeps a huge difference from asking for a grid it cannot have.
        let deep = TileId::new(12, 100, 100).expect("tile");
        let found = coarser_neighbours(deep, &|candidate| candidate.z == 2 || candidate == deep);
        assert!(found.iter().all(|levels| *levels <= MAX_STITCH_LEVELS));
        assert_eq!(found[0], MAX_STITCH_LEVELS);
    }

    /// Data that differs between levels, like unevenly loaded elevation.
    fn level_dependent(lon: f64, lat: f64, level: u8) -> Option<f64> {
        ridged(lon, lat, level).map(|height| height + 50.0 * f64::from(level))
    }

    #[test]
    fn a_corner_shared_by_tiles_of_different_levels_is_one_vertex() {
        use std::collections::HashSet;
        // A (z15) has a coarser neighbour C (z14) to the south; its east
        // neighbour B and B's south neighbour D are z15, and C does not reach
        // under B. All four meet at A's south-east corner.
        let a = TileId::new(15, 11, 11).expect("tile");
        let b = TileId::new(15, 12, 11).expect("tile");
        let d = TileId::new(15, 12, 12).expect("tile");
        let c = TileId::new(14, 5, 6).expect("tile");
        let leaves: HashSet<TileId> = [a, b, c, d].into_iter().collect();
        let is_leaf = |tile: TileId| leaves.contains(&tile);
        assert_eq!(coarser_neighbours(a, &is_leaf), [0, 0, 1, 0]);
        assert_eq!(coarser_neighbours(b, &is_leaf), [0, 0, 0, 0]);

        let cells = terrain_subdivisions(15);
        let corner = |tile: TileId, corner: usize| corner_level(tile, corner, &is_leaf);
        let build = |tile: TileId, use_corner_rule: bool| {
            let stitched = coarser_neighbours(tile, &is_leaf);
            let level = |tile: TileId, c: usize| {
                if use_corner_rule {
                    corner(tile, c)
                } else {
                    tile.z
                }
            };
            build_tile_mesh_on(
                tile,
                terrain_subdivisions(tile.z),
                Some(&level_dependent),
                stitched,
                &level,
            )
        };
        let corner_heights = |use_rule: bool| {
            let n = cells;
            // A's SE, B's SW, D's NW, and C's NE (C is z14: same 64 cells).
            [
                node(&build(a, use_rule), cells, 0, n).2,
                node(&build(b, use_rule), cells, 0, 0).2,
                node(&build(d, use_rule), cells, n, 0).2,
                node(&build(c, use_rule), cells, n, n).2,
            ]
        };
        let spread = |heights: [f64; 4]| {
            heights.iter().cloned().fold(f64::MIN, f64::max)
                - heights.iter().cloned().fold(f64::MAX, f64::min)
        };
        assert_eq!(
            corner(a, 1),
            14,
            "the coarsest touching tile sets the level"
        );
        assert!(
            spread(corner_heights(true)) < 0.05,
            "{:?}",
            corner_heights(true)
        );
        // Each sampling at its own level, they disagree by the level difference.
        assert!(
            spread(corner_heights(false)) > 40.0,
            "{:?}",
            corner_heights(false)
        );
    }

    #[test]
    fn corner_levels_follow_the_coarsest_touching_tile_only() {
        use std::collections::HashSet;
        let id = TileId::new(8, 100, 100).expect("tile");
        // A finer tile touching the corner and a missing neighbour change nothing.
        let finer = TileId::new(9, 202, 200).expect("tile");
        let alone: HashSet<TileId> = [id, finer].into_iter().collect();
        for corner in 0..4 {
            assert_eq!(corner_level(id, corner, &|tile| alone.contains(&tile)), 8);
        }
        // A level 6 tile to the west covers (96..99, 100..103) at level 8: it
        // touches both western corners, and only those.
        let west = TileId::new(6, 24, 25).expect("tile");
        let with_west: HashSet<TileId> = [id, west].into_iter().collect();
        let levels =
            [0, 1, 2, 3].map(|corner| corner_level(id, corner, &|tile| with_west.contains(&tile)));
        assert_eq!(levels, [6, 8, 6, 8]);
        // The top row has nothing to its north.
        let top = TileId::new(8, 100, 0).expect("tile");
        assert_eq!(corner_level(top, 2, &|tile| tile == top), 8);
    }

    #[test]
    fn edge_vertices_use_the_data_their_neighbours_use_when_the_next_elevation_tile_is_missing() {
        use crate::terrain::{TerrainCache, TerrainTile};
        // The imagery tile (1, 0, 0) is covered by a loaded level 1 elevation
        // tile; the one to its east is not loaded, only level 0 is. The east
        // edge lies on the boundary between them.
        let mut cache = TerrainCache::new();
        let flat = |height: f32| TerrainTile::from_heights(vec![height; 256 * 256]).expect("tile");
        cache.insert(TileId::new(0, 0, 0).expect("tile"), flat(5.0));
        cache.insert(TileId::new(1, 0, 0).expect("tile"), flat(100.0));
        let sampler = |lon: f64, lat: f64, level: u8| cache.height_at_level(lon, lat, level);
        let id = TileId::new(1, 0, 0).expect("tile");
        let mesh = mesh_on(id, 8, Some(&sampler), [0; 4]);
        for row in 0..=8 {
            for column in 0..=8 {
                let height = node(&mesh, 8, row, column).2;
                assert!(
                    (height - 100.0).abs() < 0.5,
                    "node ({row}, {column}) fell back to coarser data: {height} m"
                );
            }
        }
    }
}
