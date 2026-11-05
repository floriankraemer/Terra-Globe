//! Ellipsoid tile meshes and quadtree LOD selection.

use std::collections::BinaryHeap;

use glam::{DVec3, Mat4, Vec3, Vec4};

use crate::camera::Camera;
use crate::geo::{Geodetic, TileId, ellipsoid_normal, geodetic_to_ecef, tile_uv_rect};

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

/// Builds a grid mesh for one tile on the ellipsoid.
pub fn build_tile_mesh(id: TileId, subdivisions: u32) -> TileMesh {
    let subdivisions = subdivisions.clamp(1, 64);
    let bounds = id.bounds();
    let center = geodetic_to_ecef(bounds.center());
    let count = subdivisions + 1;

    let mut positions = Vec::with_capacity((count * count) as usize);
    let mut uvs = Vec::with_capacity((count * count) as usize);
    let mut indices = Vec::with_capacity((subdivisions * subdivisions * 6) as usize);

    for row in 0..count {
        for column in 0..count {
            let lon =
                bounds.west + bounds.width_deg() * f64::from(column) / f64::from(subdivisions);
            let lat = bounds.south + bounds.height_deg() * f64::from(row) / f64::from(subdivisions);
            let ecef = geodetic_to_ecef(Geodetic::from_degrees(lon, lat, 0.0));
            let local = (ecef - center).as_vec3();
            positions.push(local.to_array());
            uvs.push([
                f32::from(u16::try_from(column).unwrap_or(0)) / subdivisions as f32,
                1.0 - f32::from(u16::try_from(row).unwrap_or(0)) / subdivisions as f32,
            ]);
        }
    }

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

    append_skirts(
        &mut positions,
        &mut uvs,
        &mut indices,
        id,
        subdivisions,
        center,
    );
    TileMesh {
        positions,
        uvs,
        indices,
        center_ecef: center,
    }
}

/// Adds a downward skirt on all four edges. The skirt depth scales with the
/// tile's ground size, which is what makes it hide cracks at coarse levels too.
fn append_skirts(
    positions: &mut Vec<[f32; 3]>,
    uvs: &mut Vec<[f32; 2]>,
    indices: &mut Vec<u32>,
    id: TileId,
    subdivisions: u32,
    center: DVec3,
) {
    let count = subdivisions + 1;
    let skirt_depth = (id.ground_size_m() * 0.02).clamp(20.0, 200_000.0);
    let edges: [Vec<usize>; 4] = [
        (0..count).map(|column| column as usize).collect(),
        (0..count)
            .map(|column| (subdivisions * count + column) as usize)
            .collect(),
        (0..count).map(|row| (row * count) as usize).collect(),
        (0..count)
            .map(|row| (row * count + subdivisions) as usize)
            .collect(),
    ];

    for edge in edges {
        let base = positions.len() as u32;
        for &original in &edge {
            let local = Vec3::from_array(positions[original]);
            let world = center + local.as_dvec3();
            let normal = ellipsoid_normal(world).as_vec3();
            positions.push((local - normal * skirt_depth as f32).to_array());
            uvs.push(uvs[original]);
        }
        for pair in 0..edge.len().saturating_sub(1) {
            let top_a = edge[pair] as u32;
            let top_b = edge[pair + 1] as u32;
            let bottom_a = base + pair as u32;
            let bottom_b = base + pair as u32 + 1;
            indices.extend_from_slice(&[top_a, bottom_a, top_b, top_b, bottom_a, bottom_b]);
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
    if viewport.0 == 0 || viewport.1 == 0 || options.max_tiles == 0 {
        return Vec::new();
    }
    let culler = Culler::new(camera, viewport, options.texture_size);
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
struct Culler {
    eye: DVec3,
    frustum: Frustum,
    screen_height: f64,
    half_fov_tan: f64,
    texture_size: f64,
}

impl Culler {
    fn new(camera: &Camera, viewport: (u32, u32), texture_size: u32) -> Self {
        let aspect = f64::from(viewport.0) / f64::from(viewport.1);
        Self {
            eye: camera.eye_ecef(),
            frustum: Frustum::from_view_projection(camera.view_projection(aspect)),
            screen_height: f64::from(viewport.1),
            half_fov_tan: (camera.fov_y / 2.0).tan(),
            texture_size: f64::from(texture_size.max(1)),
        }
    }

    fn candidate(&self, id: TileId) -> Option<Candidate> {
        let bounds = id.bounds();
        let center = geodetic_to_ecef(bounds.center());
        let ground_size = bounds.ground_size_m();
        let radius = ground_size * 0.75;

        if !self
            .frustum
            .contains_sphere((center - self.eye).as_vec3(), radius as f32)
        {
            return None;
        }
        // Horizon test, widened by the tile's own angular size. Testing only the
        // centre drops every tile that straddles the horizon, which leaves a
        // black notch along the limb whenever the view reaches it.
        let tile_angle = radius / crate::geo::WGS84_A;
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

fn nearest_ready_texture(
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
    use crate::geo::{ecef_to_geodetic, ray_ellipsoid_point};

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
}
