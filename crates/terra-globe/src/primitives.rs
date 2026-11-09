//! Converts placemark geometry into GPU meshes: filled polygons and
//! screen-space-width polylines.
//!
//! Vertices are stored relative to the geometry centre (`center_ecef`) and the
//! renderer supplies the eye-relative translation, so nothing large reaches the
//! GPU in `f32`.

use glam::{DVec3, Vec3};
use terra_core::{GeoPoint, PlacemarkGeometry, Style};

use std::collections::HashMap;

use crate::flat::{SURFACE_Z, SceneMode, lon_lat_to_plane};
use crate::geo::{WGS84_A, ecef_to_geodetic, enu_axes, geodetic_to_ecef};

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct FillVertex {
    pub position: [f32; 3],
    pub color: [f32; 4],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LineVertex {
    pub start: [f32; 3],
    pub end: [f32; 3],
    pub side: f32,
    pub width: f32,
    /// 0 = start endpoint, 1 = end endpoint.
    pub t: f32,
    pub color: [f32; 4],
}

#[derive(Debug, Clone, Default)]
pub struct FillMesh {
    pub vertices: Vec<FillVertex>,
    pub indices: Vec<u32>,
    pub center: DVec3,
}

#[derive(Debug, Clone, Default)]
pub struct LineMesh {
    pub vertices: Vec<LineVertex>,
    pub indices: Vec<u32>,
    pub center: DVec3,
}

/// Longest edge before a ring/path is subdivided so it follows the ellipsoid.
/// At 5 km the chord sags under 0.5 m, which the surface lift below covers.
const MAX_EDGE_METERS: f64 = 5_000.0;

/// Upper bound on a filled polygon's triangle count. A continent-sized shape
/// would otherwise refine into hundreds of thousands of triangles; stopping
/// early only lets the fill sag slightly more than the 3 m lift covers.
const MAX_FILL_TRIANGLES: usize = 60_000;

/// Overlay geometry is lifted along the ellipsoid normal so it wins the depth
/// test against the globe's own tessellated surface (whose tiles are coarse
/// chords that can sit above the true ellipsoid). Three metres is invisible at
/// globe scale and small enough not to matter at street level.
const SURFACE_LIFT_METERS: f64 = 3.0;

/// Margin on the computed sag of the longest remaining fill edge.
const RESIDUAL_SAG_SAFETY: f64 = 1.25;

/// Longest edge of a line or fill that follows terrain. Terrain relief changes
/// over a few hundred metres, so the 5 km spacing that suits the ellipsoid
/// would cut straight through every ridge between two vertices.
const TERRAIN_EDGE_METERS: f64 = 200.0;

/// Upper bound on the vertices one ring or path gets from terrain densifying;
/// a long route stretches its spacing instead of exceeding it.
const MAX_TERRAIN_PATH_VERTICES: usize = 20_000;

/// Lift over terrain. The terrain tiles are a coarser surface than the heights
/// the geometry samples, so a few more metres than over the ellipsoid keeps
/// lines and fills from sinking into a slope between two grid vertices.
const TERRAIN_LIFT_METERS: f64 = 6.0;

/// Spacing and lift for one geometry: fine and higher where terrain is loaded.
#[derive(Debug, Clone, Copy)]
struct Detail {
    max_edge_m: f64,
    lift_m: f64,
    terrain: bool,
}

impl Detail {
    const ELLIPSOID: Self = Self {
        max_edge_m: MAX_EDGE_METERS,
        lift_m: SURFACE_LIFT_METERS,
        terrain: false,
    };
    const TERRAIN: Self = Self {
        max_edge_m: TERRAIN_EDGE_METERS,
        lift_m: TERRAIN_LIFT_METERS,
        terrain: true,
    };

    /// Terrain detail where the lookup knows the ground under the geometry.
    fn for_geometry(geometry: &PlacemarkGeometry, ground: &crate::geo::GroundFn<'_>) -> Self {
        let center = terra_core::geometry_center(geometry);
        if ground(center.lon, center.lat).is_some() {
            Self::TERRAIN
        } else {
            Self::ELLIPSOID
        }
    }
}

pub fn build_meshes(
    geometry: &PlacemarkGeometry,
    style: &Style,
    selected: bool,
    mode: SceneMode,
    ground: &crate::geo::GroundFn<'_>,
) -> (Option<FillMesh>, Option<LineMesh>) {
    if mode.is_flat() {
        return build_flat_meshes(geometry, style, selected);
    }
    let center = crate::geo::geo_point_to_ecef(&terra_core::geometry_center(geometry), ground);
    let detail = Detail::for_geometry(geometry, ground);
    let (outline_color, outline_width) = outline_style(style, selected);
    let fill_color = fill_color(style);

    let (rings, closed) = match geometry {
        PlacemarkGeometry::Polygon {
            outer_ring,
            inner_rings,
            ..
        } => {
            let mut rings = vec![densify_ring_with(outer_ring, detail)];
            if let Some(inner) = inner_rings {
                rings.extend(inner.iter().map(|ring| densify_ring_with(ring, detail)));
            }
            (rings, true)
        }
        PlacemarkGeometry::Rectangle {
            north,
            south,
            east,
            west,
        } => (
            vec![densify_ring_with(
                &rectangle_ring(*north, *south, *east, *west),
                detail,
            )],
            true,
        ),
        PlacemarkGeometry::Circle {
            center: circle_center,
            radius_meters,
        } => (
            vec![densify_ring_with(
                &terra_core::circle_to_polygon_ring(circle_center, *radius_meters, 96),
                detail,
            )],
            true,
        ),
        PlacemarkGeometry::GroundOverlay { bounds, .. } => (
            vec![densify_ring_with(
                &rectangle_ring(bounds.north, bounds.south, bounds.east, bounds.west),
                detail,
            )],
            true,
        ),
        PlacemarkGeometry::LineString { path, .. } => {
            (vec![densify_path_with(path, detail)], false)
        }
        PlacemarkGeometry::Point { .. } | PlacemarkGeometry::Model { .. } => (Vec::new(), false),
    };

    if rings.is_empty() {
        return (None, None);
    }

    let line = outline_width.map(|width| {
        let mut mesh = LineMesh {
            center,
            ..Default::default()
        };
        let points: Vec<Vec<Vec3>> = rings
            .iter()
            .map(|ring| {
                ring.iter()
                    .map(|point| ecef_local(point, center, ground, detail.lift_m))
                    .collect()
            })
            .collect();
        for ring in &points {
            append_polyline(&mut mesh, ring, width as f32, outline_color, closed);
        }
        mesh
    });

    let fill = if closed && style.filled {
        Some(build_fill(&rings, center, fill_color, ground, detail))
    } else if closed && matches!(geometry, PlacemarkGeometry::GroundOverlay { .. }) {
        // Ground overlays always show their extent even without a style fill.
        Some(build_fill(
            &rings,
            center,
            [1.0, 1.0, 1.0, 0.18],
            ground,
            detail,
        ))
    } else {
        None
    };

    (fill, line)
}

/// Local ECEF position with the surface lift applied.
fn ecef_local(
    point: &GeoPoint,
    center: DVec3,
    ground: &crate::geo::GroundFn<'_>,
    lift_m: f64,
) -> Vec3 {
    let local = (crate::geo::geo_point_to_ecef(point, ground) - center).as_vec3();
    lift(local, center, lift_m)
}

/// Flat-plane variant: positions are plane meters with a fixed lift.
fn build_flat_meshes(
    geometry: &PlacemarkGeometry,
    style: &Style,
    selected: bool,
) -> (Option<FillMesh>, Option<LineMesh>) {
    let center_plane = lon_lat_to_plane(&terra_core::geometry_center(geometry));
    let (outline_color, outline_width) = outline_style(style, selected);
    let fill_color = fill_color(style);

    let rings_geo: Vec<Vec<GeoPoint>> = match geometry {
        PlacemarkGeometry::Polygon {
            outer_ring,
            inner_rings,
            ..
        } => {
            let mut rings = vec![outer_ring.clone()];
            if let Some(inner) = inner_rings {
                rings.extend(inner.clone());
            }
            rings
        }
        PlacemarkGeometry::Rectangle {
            north,
            south,
            east,
            west,
        } => vec![rectangle_ring(*north, *south, *east, *west)],
        PlacemarkGeometry::Circle {
            center,
            radius_meters,
        } => vec![terra_core::circle_to_polygon_ring(
            center,
            *radius_meters,
            96,
        )],
        PlacemarkGeometry::GroundOverlay { bounds, .. } => vec![rectangle_ring(
            bounds.north,
            bounds.south,
            bounds.east,
            bounds.west,
        )],
        PlacemarkGeometry::LineString { path, .. } => vec![path.clone()],
        PlacemarkGeometry::Point { .. } | PlacemarkGeometry::Model { .. } => Vec::new(),
    };
    if rings_geo.is_empty() {
        return (None, None);
    }
    let closed = !matches!(geometry, PlacemarkGeometry::LineString { .. });
    let plane_local = |point: &GeoPoint| -> Vec3 {
        let plane = lon_lat_to_plane(point);
        Vec3::new(
            (plane.x - center_plane.x) as f32,
            (plane.y - center_plane.y) as f32,
            SURFACE_Z as f32,
        )
    };
    let rings: Vec<Vec<Vec3>> = rings_geo
        .iter()
        .map(|ring| ring.iter().map(plane_local).collect())
        .collect();

    let line = outline_width.map(|width| {
        let mut mesh = LineMesh {
            center: center_plane,
            ..Default::default()
        };
        for ring in &rings {
            append_polyline(&mut mesh, ring, width as f32, outline_color, closed);
        }
        mesh
    });

    let fill = if closed && style.filled {
        Some(build_flat_fill(&rings, center_plane, fill_color))
    } else if closed && matches!(geometry, PlacemarkGeometry::GroundOverlay { .. }) {
        Some(build_flat_fill(&rings, center_plane, [1.0, 1.0, 1.0, 0.18]))
    } else {
        None
    };
    (fill, line)
}

fn build_flat_fill(rings: &[Vec<Vec3>], center: DVec3, color: [f32; 4]) -> FillMesh {
    let mut flat: Vec<f64> = Vec::new();
    let mut holes: Vec<usize> = Vec::new();
    let mut world: Vec<Vec3> = Vec::new();
    for (index, ring) in rings.iter().enumerate() {
        if ring.len() < 3 {
            continue;
        }
        if index > 0 {
            holes.push(flat.len() / 2);
        }
        for point in ring {
            flat.push(f64::from(point.x));
            flat.push(f64::from(point.y));
            world.push(*point);
        }
    }
    let Ok(indices) = earcutr::earcut(&flat, &holes, 2) else {
        return FillMesh::default();
    };
    FillMesh {
        vertices: world
            .into_iter()
            .map(|position| FillVertex {
                position: position.to_array(),
                color,
            })
            .collect(),
        indices: indices.into_iter().map(|index| index as u32).collect(),
        center,
    }
}

fn outline_style(style: &Style, selected: bool) -> ([f32; 4], Option<f64>) {
    if selected {
        return (
            [1.0, 0.85, 0.35, 1.0],
            Some(style.outline_width.max(1.0) + 2.5),
        );
    }
    if !style.outline_enabled {
        return ([0.0, 0.0, 0.0, 0.0], None);
    }
    let color = parse_color(&style.outline_color, 1.0);
    (color, Some(style.outline_width.max(0.5)))
}

fn fill_color(style: &Style) -> [f32; 4] {
    parse_color(&style.fill_color, style.fill_opacity)
}

pub fn parse_color(hex: &str, opacity: f64) -> [f32; 4] {
    let channel = |range: std::ops::Range<usize>| {
        u8::from_str_radix(hex.get(range).unwrap_or("ff"), 16).unwrap_or(255) as f32 / 255.0
    };
    [
        channel(1..3),
        channel(3..5),
        channel(5..7),
        opacity.clamp(0.0, 1.0) as f32,
    ]
}

/// Moves a local vertex outward along the ellipsoid normal by `height` meters.
fn lift(local: Vec3, center: DVec3, height: f64) -> Vec3 {
    let world = center + local.as_dvec3();
    let normal = crate::geo::ellipsoid_normal(world).as_vec3();
    local + normal * height as f32
}

fn rectangle_ring(north: f64, south: f64, east: f64, west: f64) -> Vec<GeoPoint> {
    let point = |lon: f64, lat: f64| GeoPoint {
        lon,
        lat,
        altitude: None,
        altitude_mode: None,
    };
    vec![
        point(west, south),
        point(east, south),
        point(east, north),
        point(west, north),
        point(west, south),
    ]
}

/// Subdivides a closed ring so long edges follow the ellipsoid. The closing
/// vertex is preserved.
pub fn densify_ring(ring: &[GeoPoint]) -> Vec<GeoPoint> {
    densify_ring_with(ring, Detail::ELLIPSOID)
}

fn densify_ring_with(ring: &[GeoPoint], detail: Detail) -> Vec<GeoPoint> {
    if ring.len() < 2 {
        return ring.to_vec();
    }
    let max_edge = edge_for_budget(ring, true, detail);
    let mut out = Vec::with_capacity(ring.len());
    for pair in ring.windows(2) {
        append_segment(&mut out, &pair[0], &pair[1], max_edge);
    }
    if let (Some(first), Some(last)) = (ring.first(), ring.last()) {
        if first.lon != last.lon || first.lat != last.lat {
            append_segment(&mut out, last, first, max_edge);
        }
    }
    out.push(*ring.last().expect("non-empty ring"));
    out
}

pub fn densify_path(path: &[GeoPoint]) -> Vec<GeoPoint> {
    densify_path_with(path, Detail::ELLIPSOID)
}

fn densify_path_with(path: &[GeoPoint], detail: Detail) -> Vec<GeoPoint> {
    if path.len() < 2 {
        return path.to_vec();
    }
    let max_edge = edge_for_budget(path, false, detail);
    let mut out = Vec::with_capacity(path.len());
    for pair in path.windows(2) {
        append_segment(&mut out, &pair[0], &pair[1], max_edge);
    }
    if let Some(last) = path.last() {
        out.push(*last);
    }
    out
}

/// The edge length to densify to: the detail's own, stretched for terrain so a
/// very long ring or path stays within [`MAX_TERRAIN_PATH_VERTICES`].
fn edge_for_budget(points: &[GeoPoint], closed: bool, detail: Detail) -> f64 {
    if !detail.terrain {
        return detail.max_edge_m;
    }
    let mut length: f64 = points
        .windows(2)
        .map(|pair| terra_core::haversine_meters(&pair[0], &pair[1]))
        .sum();
    if closed {
        if let (Some(first), Some(last)) = (points.first(), points.last()) {
            length += terra_core::haversine_meters(last, first);
        }
    }
    detail
        .max_edge_m
        .max(length / MAX_TERRAIN_PATH_VERTICES as f64)
}

fn append_segment(out: &mut Vec<GeoPoint>, a: &GeoPoint, b: &GeoPoint, max_edge: f64) {
    let distance = terra_core::haversine_meters(a, b);
    let steps = ((distance / max_edge).ceil() as usize).max(1);
    out.push(*a);
    for step in 1..steps {
        out.push(interpolate(a, b, step as f64 / steps as f64));
    }
}

/// Great-circle interpolation so antimeridian crossings take the short way.
fn interpolate(a: &GeoPoint, b: &GeoPoint, t: f64) -> GeoPoint {
    let start = geodetic_to_ecef(crate::geo::Geodetic::from_degrees(a.lon, a.lat, 0.0)).normalize();
    let end = geodetic_to_ecef(crate::geo::Geodetic::from_degrees(b.lon, b.lat, 0.0)).normalize();
    let dot = start.dot(end).clamp(-1.0, 1.0);
    let angle = dot.acos();
    let point = if angle < 1e-9 {
        start
    } else if (std::f64::consts::PI - angle).abs() < 1e-6 {
        start.lerp(end, t).normalize()
    } else {
        let sin_angle = angle.sin();
        start * ((1.0 - t) * angle).sin() / sin_angle + end * (t * angle).sin() / sin_angle
    };
    let geodetic = ecef_to_geodetic(point * WGS84_A);
    let mut point = GeoPoint {
        lon: geodetic.lon_deg(),
        lat: geodetic.lat_deg(),
        altitude: match (a.altitude, b.altitude) {
            (Some(start), Some(end)) => Some(start + (end - start) * t),
            _ => None,
        },
        altitude_mode: a.altitude_mode,
    };
    point.lon = point.lon.clamp(-180.0, 180.0);
    point.lat = point.lat.clamp(-90.0, 90.0);
    point
}

fn append_polyline(
    mesh: &mut LineMesh,
    points: &[Vec3],
    width: f32,
    color: [f32; 4],
    closed: bool,
) {
    if points.len() < 2 {
        return;
    }

    let segments = if closed {
        points.len()
    } else {
        points.len().saturating_sub(1)
    };
    for index in 0..segments {
        let start = points[index];
        let end = points[(index + 1) % points.len()];
        if start.distance_squared(end) < 1e-6 {
            continue;
        }
        let base = mesh.vertices.len() as u32;
        for (side, t) in [(-1.0f32, 0.0f32), (1.0, 0.0), (-1.0, 1.0), (1.0, 1.0)] {
            mesh.vertices.push(LineVertex {
                start: start.to_array(),
                end: end.to_array(),
                side,
                width,
                t,
                color,
            });
        }
        mesh.indices
            .extend_from_slice(&[base, base + 1, base + 2, base + 2, base + 1, base + 3]);
    }
}

fn build_fill(
    rings: &[Vec<GeoPoint>],
    center: DVec3,
    color: [f32; 4],
    ground: &crate::geo::GroundFn<'_>,
    detail: Detail,
) -> FillMesh {
    let (east, north, _) = enu_axes(center);

    let mut flat: Vec<f64> = Vec::new();
    let mut holes: Vec<usize> = Vec::new();
    let mut world: Vec<Vec3> = Vec::new();
    let mut geo: Vec<GeoPoint> = Vec::new();

    for (ring_index, ring) in rings.iter().enumerate() {
        if ring.len() < 3 {
            continue;
        }
        if ring_index > 0 {
            holes.push(flat.len() / 2);
        }
        for point in ring {
            let ecef = crate::geo::geo_point_to_ecef(point, ground);
            let local = ecef - center;
            flat.push(local.dot(east));
            flat.push(local.dot(north));
            world.push(local.as_vec3());
            geo.push(*point);
        }
    }

    if world.is_empty() {
        return FillMesh::default();
    }

    let Ok(indices) = earcutr::earcut(&flat, &holes, 2) else {
        return FillMesh::default();
    };
    let indices: Vec<u32> = indices.into_iter().map(|index| index as u32).collect();
    // Earcut only triangulates the boundary, so on a curved surface a single
    // triangle can span hundreds of kilometres and sag far below the globe -
    // the fill then disappears where the globe occludes it. Refine the
    // triangles until their edges are short enough to follow the ellipsoid.
    let indices = refine_fill(&mut world, &mut geo, &indices, center, ground, detail);

    // Where the triangle budget stopped refinement, long edges remain. Their
    // interiors sag below the ellipsoid by up to edge^2 / (6R) (centroid of an
    // equilateral triangle), so lift the whole fill by that much on top of the
    // usual surface lift; it is a no-op for fully refined shapes.
    let longest = indices
        .chunks_exact(3)
        .flat_map(|t| [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])])
        .map(|key| edge_length(&world, key))
        .fold(0.0, f64::max);
    let height =
        detail.lift_m + RESIDUAL_SAG_SAFETY * longest * longest / (6.0 * crate::geo::WGS84_A);

    FillMesh {
        vertices: world
            .into_iter()
            .map(|position| FillVertex {
                position: lift(position, center, height).to_array(),
                color,
            })
            .collect(),
        indices,
        center,
    }
}

/// Conforming edge split of a triangulation. Every shared edge is split once,
/// so neighbouring triangles keep matching vertices and no cracks appear.
fn refine_fill(
    vertices: &mut Vec<Vec3>,
    geo: &mut Vec<GeoPoint>,
    indices: &[u32],
    center: DVec3,
    ground: &crate::geo::GroundFn<'_>,
    detail: Detail,
) -> Vec<u32> {
    let mut triangles: Vec<[u32; 3]> = indices
        .chunks_exact(3)
        .filter(|chunk| chunk.len() == 3)
        .map(|chunk| [chunk[0], chunk[1], chunk[2]])
        .collect();
    let mut midpoints: HashMap<(u32, u32), u32> = HashMap::new();

    loop {
        // Longest edges first, as many as the triangle budget still allows:
        // a split edge adds one triangle per adjacent triangle.
        let mut candidates: HashMap<(u32, u32), (f64, usize)> = HashMap::new();
        for [a, b, c] in &triangles {
            for (start, end) in [(*a, *b), (*b, *c), (*c, *a)] {
                let key = edge_key(start, end);
                let length = edge_length(vertices, key);
                if length > detail.max_edge_m {
                    candidates.entry(key).or_insert((length, 0)).1 += 1;
                }
            }
        }
        let mut candidates: Vec<_> = candidates.into_iter().collect();
        candidates.sort_unstable_by(|a, b| b.1.0.total_cmp(&a.1.0).then(a.0.cmp(&b.0)));
        let mut budget = MAX_FILL_TRIANGLES.saturating_sub(triangles.len());
        let mut long_edges: Vec<(u32, u32)> = Vec::new();
        for (key, (_, adjacent)) in candidates {
            if adjacent > budget {
                break;
            }
            budget -= adjacent;
            long_edges.push(key);
        }
        if long_edges.is_empty() {
            break;
        }

        for &(start, end) in &long_edges {
            midpoints.entry((start, end)).or_insert_with(|| {
                let index = vertices.len() as u32;
                let point = interpolate(&geo[start as usize], &geo[end as usize], 0.5);
                let ecef = crate::geo::geo_point_to_ecef(&point, ground);
                vertices.push((ecef - center).as_vec3());
                geo.push(point);
                index
            });
        }

        let mut refined = Vec::with_capacity(triangles.len() * 4);
        for [a, b, c] in &triangles {
            let ab = midpoint(&midpoints, *a, *b);
            let bc = midpoint(&midpoints, *b, *c);
            let ca = midpoint(&midpoints, *c, *a);
            split_triangle(*a, *b, *c, ab, bc, ca, &mut refined);
        }
        triangles = refined;
    }

    triangles.into_iter().flatten().collect()
}

fn edge_key(a: u32, b: u32) -> (u32, u32) {
    if a < b { (a, b) } else { (b, a) }
}

fn edge_length(vertices: &[Vec3], key: (u32, u32)) -> f64 {
    f64::from(vertices[key.0 as usize].distance(vertices[key.1 as usize]))
}

fn midpoint(midpoints: &HashMap<(u32, u32), u32>, a: u32, b: u32) -> Option<u32> {
    midpoints.get(&edge_key(a, b)).copied()
}

fn split_triangle(
    a: u32,
    b: u32,
    c: u32,
    ab: Option<u32>,
    bc: Option<u32>,
    ca: Option<u32>,
    out: &mut Vec<[u32; 3]>,
) {
    match (ab, bc, ca) {
        (None, None, None) => out.push([a, b, c]),
        (Some(m), None, None) => out.extend([[a, m, c], [m, b, c]]),
        (None, Some(m), None) => out.extend([[a, b, m], [a, m, c]]),
        (None, None, Some(m)) => out.extend([[a, b, m], [b, c, m]]),
        (Some(m0), Some(m1), None) => out.extend([[a, m0, m1], [m0, b, m1], [a, m1, c]]),
        (None, Some(m1), Some(m2)) => out.extend([[a, b, m1], [a, m1, m2], [m1, c, m2]]),
        (Some(m0), None, Some(m2)) => out.extend([[m0, b, c], [m0, c, m2], [m0, m2, a]]),
        (Some(m0), Some(m1), Some(m2)) => {
            out.extend([[a, m0, m2], [m0, b, m1], [m2, m1, c], [m0, m1, m2]])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_core::{NewStyle, RectangleBounds, create_style};

    fn point(lon: f64, lat: f64) -> GeoPoint {
        GeoPoint {
            lon,
            lat,
            altitude: None,
            altitude_mode: None,
        }
    }

    fn style() -> Style {
        create_style(NewStyle {
            outline_color: "#3366cc".into(),
            outline_width: 2.0,
            fill_color: "#cc6633".into(),
            fill_opacity: 0.6,
            outline_enabled: None,
            filled: Some(true),
            icon_url: None,
            icon_scale: None,
            marker_icon: None,
            label_color: None,
        })
        .expect("style")
    }

    #[test]
    fn polygon_produces_fill_and_outline_meshes() {
        let geometry = PlacemarkGeometry::polygon(
            vec![
                point(0.0, 0.0),
                point(0.1, 0.0),
                point(0.1, 0.1),
                point(0.0, 0.1),
            ],
            None,
            None,
            None,
        )
        .expect("polygon");
        let (fill, line) = build_meshes(
            &geometry,
            &style(),
            false,
            crate::flat::SceneMode::Scene3D,
            &crate::geo::no_ground,
        );
        let fill = fill.expect("filled polygon");
        assert!(!fill.vertices.is_empty());
        assert_eq!(fill.indices.len() % 3, 0);
        let line = line.expect("outline");
        assert!(!line.vertices.is_empty());
    }

    fn edge_height(center: DVec3, local: [f32; 3]) -> f64 {
        let local = DVec3::new(
            f64::from(local[0]),
            f64::from(local[1]),
            f64::from(local[2]),
        );
        crate::geo::ecef_to_geodetic(center + local).height
    }

    fn imported_ring(mode: Option<terra_core::AltitudeMode>) -> PlacemarkGeometry {
        let at = |lon: f64, lat: f64| GeoPoint {
            lon,
            lat,
            altitude: Some(490.0),
            altitude_mode: mode,
        };
        PlacemarkGeometry::line_string(
            vec![
                at(9.9396, 51.1446),
                at(9.9496, 51.1446),
                at(9.9496, 51.1546),
            ],
            None,
            None,
        )
        .expect("line")
    }

    #[test]
    fn imported_altitude_does_not_float_without_terrain() {
        use terra_core::AltitudeMode::{Absolute, ClampToGround, RelativeToGround};
        for mode in [
            None,
            Some(ClampToGround),
            Some(Absolute),
            Some(RelativeToGround),
        ] {
            let (_, line) = build_meshes(
                &imported_ring(mode),
                &style(),
                false,
                crate::flat::SceneMode::Scene3D,
                &crate::geo::no_ground,
            );
            let line = line.expect("line");
            assert!(!line.vertices.is_empty());
            for vertex in &line.vertices {
                let height = edge_height(line.center, vertex.start);
                assert!(
                    (height - SURFACE_LIFT_METERS).abs() < 1.0,
                    "{mode:?}: vertex at {height} m"
                );
            }
        }
    }

    #[test]
    fn terrain_height_is_honoured_when_supplied() {
        let (_, line) = build_meshes(
            &imported_ring(None),
            &style(),
            false,
            crate::flat::SceneMode::Scene3D,
            &|_, _| Some(300.0),
        );
        let line = line.expect("line");
        let height = edge_height(line.center, line.vertices[0].start);
        assert!(
            (height - 300.0 - TERRAIN_LIFT_METERS).abs() < 1.0,
            "{height}"
        );
    }

    fn hill(lon: f64, lat: f64) -> Option<f64> {
        // A 1.5 km dome centred on the middle of the ring from `imported_ring`.
        let d = ((lon - 9.9446).hypot(lat - 51.1496)).to_radians() * crate::geo::WGS84_A;
        Some((1_500.0 - d * 0.3).max(0.0))
    }

    #[test]
    fn clamped_lines_follow_the_terrain_on_a_fine_grid() {
        let geometry = imported_ring(Some(terra_core::AltitudeMode::ClampToGround));
        let mode = crate::flat::SceneMode::Scene3D;
        let (_, flat) = build_meshes(&geometry, &style(), false, mode, &crate::geo::no_ground);
        let (_, draped) = build_meshes(&geometry, &style(), false, mode, &hill);
        let (flat, draped) = (flat.expect("line"), draped.expect("line"));
        assert!(
            draped.vertices.len() > flat.vertices.len() * 4,
            "terrain needs a finer grid: {} vs {}",
            draped.vertices.len(),
            flat.vertices.len()
        );
        let mut checked = 0;
        for vertex in &draped.vertices {
            let world = draped.center
                + DVec3::new(
                    f64::from(vertex.start[0]),
                    f64::from(vertex.start[1]),
                    f64::from(vertex.start[2]),
                );
            let geodetic = crate::geo::ecef_to_geodetic(world);
            let ground = hill(geodetic.lon_deg(), geodetic.lat_deg()).expect("ground");
            let clearance = geodetic.height - ground;
            assert!(
                (clearance - TERRAIN_LIFT_METERS).abs() < 1.5,
                "line is {clearance} m above the ground, wanted ~{TERRAIN_LIFT_METERS}"
            );
            checked += 1;
        }
        assert!(checked > 30);
        let summit = draped
            .vertices
            .iter()
            .map(|v| edge_height(draped.center, v.start))
            .fold(0.0, f64::max);
        assert!(summit > 1_000.0, "the line climbs the hill: {summit}");
    }

    #[test]
    fn fills_over_terrain_stay_above_it_and_get_finer() {
        let ring = vec![
            point(9.94, 51.144),
            point(9.95, 51.144),
            point(9.95, 51.155),
            point(9.94, 51.155),
        ];
        let geometry = PlacemarkGeometry::polygon(ring, None, None, None).expect("polygon");
        let mode = crate::flat::SceneMode::Scene3D;
        let (flat, _) = build_meshes(&geometry, &style(), false, mode, &crate::geo::no_ground);
        let (draped, _) = build_meshes(&geometry, &style(), false, mode, &hill);
        let (flat, draped) = (flat.expect("fill"), draped.expect("fill"));
        assert!(draped.indices.len() > flat.indices.len() * 4);
        for vertex in &draped.vertices {
            let world = draped.center
                + DVec3::new(
                    f64::from(vertex.position[0]),
                    f64::from(vertex.position[1]),
                    f64::from(vertex.position[2]),
                );
            let geodetic = crate::geo::ecef_to_geodetic(world);
            let ground = hill(geodetic.lon_deg(), geodetic.lat_deg()).expect("ground");
            assert!(geodetic.height >= ground + TERRAIN_LIFT_METERS - 1.0);
        }
    }

    #[test]
    fn terrain_densifying_stays_within_the_vertex_budget() {
        // About 9,000 km of path: at 200 m that is 45,000 segments; the budget
        // stretches the spacing instead.
        let path = vec![point(-60.0, 0.0), point(20.0, 0.0)];
        let detail = Detail::TERRAIN;
        let dense = densify_path_with(&path, detail);
        assert!(
            dense.len() <= MAX_TERRAIN_PATH_VERTICES + 100,
            "{}",
            dense.len()
        );
        assert!(dense.len() > 1_000);
        // A short path keeps the full resolution.
        let short = densify_path_with(&[point(9.0, 51.0), point(9.01, 51.0)], detail);
        let edge = terra_core::haversine_meters(&short[0], &short[1]);
        assert!(edge <= TERRAIN_EDGE_METERS + 1.0 && short.len() > 3);
    }

    #[test]
    fn without_terrain_the_ellipsoid_spacing_and_lift_are_unchanged() {
        let geometry = imported_ring(Some(terra_core::AltitudeMode::ClampToGround));
        let (_, line) = build_meshes(
            &geometry,
            &style(),
            false,
            crate::flat::SceneMode::Scene3D,
            &crate::geo::no_ground,
        );
        let line = line.expect("line");
        // Two ~700 m / ~1.1 km legs fit in single 5 km steps.
        assert_eq!(line.vertices.len(), 2 * 4);
    }

    #[test]
    fn polygon_with_hole_triangulates_the_ring_and_the_hole() {
        let outer = vec![
            point(0.0, 0.0),
            point(0.2, 0.0),
            point(0.2, 0.2),
            point(0.0, 0.2),
        ];
        let hole = vec![
            point(0.08, 0.08),
            point(0.12, 0.08),
            point(0.12, 0.12),
            point(0.08, 0.12),
        ];
        let geometry =
            PlacemarkGeometry::polygon(outer, Some(vec![hole]), None, None).expect("polygon");
        let (fill, _) = build_meshes(
            &geometry,
            &style(),
            false,
            crate::flat::SceneMode::Scene3D,
            &crate::geo::no_ground,
        );
        let fill = fill.expect("fill");
        assert!(fill.vertices.len() >= 8);
        assert!(fill.indices.len() >= 18);
    }

    #[test]
    fn unfilled_shapes_get_only_an_outline() {
        let mut style = style();
        style.filled = false;
        let geometry = PlacemarkGeometry::circle(point(0.0, 0.0), 500.0).expect("circle");
        let (fill, line) = build_meshes(
            &geometry,
            &style,
            false,
            crate::flat::SceneMode::Scene3D,
            &crate::geo::no_ground,
        );
        assert!(fill.is_none());
        assert!(line.is_some());
    }

    #[test]
    fn hidden_outline_produces_no_line_mesh() {
        let mut style = style();
        style.outline_enabled = false;
        let geometry =
            PlacemarkGeometry::rectangle(RectangleBounds::new(1.0, 0.0, 1.0, 0.0).expect("bounds"))
                .expect("rectangle");
        let (_, line) = build_meshes(
            &geometry,
            &style,
            false,
            crate::flat::SceneMode::Scene3D,
            &crate::geo::no_ground,
        );
        assert!(line.is_none());
    }

    #[test]
    fn lines_are_densified_for_long_segments() {
        let path = vec![point(0.0, 0.0), point(0.0, 3.0)];
        let dense = densify_path(&path);
        assert!(
            dense.len() > 4,
            "expected densification, got {}",
            dense.len()
        );
        assert!((dense[0].lat - 0.0).abs() < 1e-9);
        assert!((dense.last().expect("last").lat - 3.0).abs() < 1e-9);
    }

    #[test]
    fn antimeridian_interpolation_takes_the_short_way() {
        let dense = densify_path(&[point(179.0, 0.0), point(-179.0, 0.0)]);
        let middle = &dense[dense.len() / 2];
        assert!(
            middle.lon.abs() > 179.0,
            "interpolated lon {} should stay near the antimeridian",
            middle.lon
        );
    }

    #[test]
    fn selected_shapes_get_a_wider_highlight_outline() {
        let geometry =
            PlacemarkGeometry::line_string(vec![point(0.0, 0.0), point(0.1, 0.1)], None, None)
                .expect("line");
        let (_, normal) = build_meshes(
            &geometry,
            &style(),
            false,
            crate::flat::SceneMode::Scene3D,
            &crate::geo::no_ground,
        );
        let (_, selected) = build_meshes(
            &geometry,
            &style(),
            true,
            crate::flat::SceneMode::Scene3D,
            &crate::geo::no_ground,
        );
        let normal_width = normal.expect("line").vertices[0].width;
        let selected_width = selected.expect("line").vertices[0].width;
        assert!(selected_width > normal_width);
    }

    fn scene_3d_fill(geometry: &PlacemarkGeometry) -> Option<FillMesh> {
        build_meshes(
            geometry,
            &style(),
            false,
            crate::flat::SceneMode::Scene3D,
            &crate::geo::no_ground,
        )
        .0
    }

    fn rect_ring(west: f64, south: f64, east: f64, north: f64) -> Vec<GeoPoint> {
        vec![
            point(west, south),
            point(east, south),
            point(east, north),
            point(west, north),
            point(west, south),
        ]
    }

    fn height_of(fill: &FillMesh, local: Vec3) -> f64 {
        ecef_to_geodetic(fill.center + local.as_dvec3()).height
    }

    /// Lowest height over triangle centroids and edge midpoints: the places a
    /// flat triangle sags furthest below the ellipsoid.
    fn lowest_surface_height(fill: &FillMesh) -> f64 {
        let vertex = |index: u32| Vec3::from_array(fill.vertices[index as usize].position);
        let mut lowest = f64::INFINITY;
        for t in fill.indices.chunks_exact(3) {
            let (a, b, c) = (vertex(t[0]), vertex(t[1]), vertex(t[2]));
            for sample in [
                (a + b + c) / 3.0,
                (a + b) / 2.0,
                (b + c) / 2.0,
                (c + a) / 2.0,
            ] {
                lowest = lowest.min(height_of(fill, sample));
            }
        }
        lowest
    }

    fn fill_shapes() -> Vec<(&'static str, PlacemarkGeometry)> {
        let circle = |radius| PlacemarkGeometry::circle(point(10.0, 35.0), radius).expect("circle");
        vec![
            ("small circle", circle(500.0)),
            ("150 km circle", circle(150_000.0)),
            ("2000 km circle", circle(2_000_000.0)),
            (
                "capped continent",
                PlacemarkGeometry::polygon(rect_ring(-10.0, 38.0, 35.0, 62.0), None, None, None)
                    .expect("continent"),
            ),
            (
                "polygon with hole",
                PlacemarkGeometry::polygon(
                    rect_ring(-5.0, 40.0, 25.0, 60.0),
                    Some(vec![rect_ring(5.0, 45.0, 15.0, 55.0)]),
                    None,
                    None,
                )
                .expect("polygon with hole"),
            ),
        ]
    }

    #[test]
    fn fills_stay_above_the_ellipsoid() {
        for (name, geometry) in fill_shapes() {
            let fill = scene_3d_fill(&geometry).unwrap_or_else(|| panic!("{name}: no fill"));
            assert!(!fill.indices.is_empty(), "{name}: empty fill");
            let lowest = lowest_surface_height(&fill);
            assert!(
                lowest > 0.0,
                "{name} sinks {lowest:.2} m below the ellipsoid"
            );
        }
    }

    #[test]
    fn fills_do_not_float_far_above_the_surface() {
        for (name, geometry) in fill_shapes() {
            let fill = scene_3d_fill(&geometry).expect("fill");
            let highest = fill
                .vertices
                .iter()
                .map(|v| height_of(&fill, Vec3::from_array(v.position)))
                .fold(f64::NEG_INFINITY, f64::max);
            // Shapes that fit the triangle budget only carry the plain lift;
            // budget-limited ones carry their residual sag (kilometres for a
            // continent, invisible at the zoom levels where it is visible).
            let limit = if fill.indices.len() / 3 < MAX_FILL_TRIANGLES / 2 {
                SURFACE_LIFT_METERS + 1.0
            } else {
                3_000.0
            };
            assert!(
                highest < limit,
                "{name} floats {highest:.1} m above the surface"
            );
        }
    }

    #[test]
    fn unlifted_planar_fill_would_sink_which_is_what_the_lift_prevents() {
        let geometry = PlacemarkGeometry::circle(point(10.0, 35.0), 2_000_000.0).expect("circle");
        let fill = scene_3d_fill(&geometry).expect("fill");
        let bare = SURFACE_LIFT_METERS;
        let lowest = lowest_surface_height(&fill);
        assert!(
            lowest > bare,
            "capped fill must be lifted beyond the plain {bare} m ({lowest:.2} m)"
        );
    }

    #[test]
    fn fill_tessellation_is_bounded() {
        for (name, geometry) in fill_shapes() {
            let fill = scene_3d_fill(&geometry).expect("fill");
            assert!(
                fill.indices.len() / 3 <= MAX_FILL_TRIANGLES,
                "{name} produced {} triangles",
                fill.indices.len() / 3
            );
        }
        let huge = PlacemarkGeometry::circle(point(10.0, 35.0), 1_500_000.0).expect("circle");
        let fill = scene_3d_fill(&huge).expect("fill");
        assert!(fill.indices.len() / 3 <= MAX_FILL_TRIANGLES);
    }

    #[test]
    fn small_shapes_are_not_over_tessellated() {
        let geometry = PlacemarkGeometry::circle(point(10.0, 35.0), 500.0).expect("circle");
        let fill = scene_3d_fill(&geometry).expect("fill");
        assert!(
            fill.indices.len() / 3 < 100,
            "{} triangles",
            fill.indices.len() / 3
        );
    }

    #[test]
    fn one_meter_circle_still_fills() {
        let geometry = PlacemarkGeometry::circle(point(10.0, 35.0), 1.0).expect("circle");
        let fill = scene_3d_fill(&geometry).expect("fill");
        assert!(!fill.indices.is_empty());
        assert!(
            fill.vertices
                .iter()
                .all(|v| v.position.iter().all(|c| c.is_finite()))
        );
        assert!(lowest_surface_height(&fill) > 0.0);
    }

    #[test]
    fn degenerate_rings_produce_an_empty_fill() {
        let center = geodetic_to_ecef(crate::geo::Geodetic::from_degrees(10.0, 35.0, 0.0));
        let color = [1.0; 4];
        let two_points = vec![point(10.0, 35.0), point(10.1, 35.0)];
        assert!(
            build_fill(
                &[two_points],
                center,
                color,
                &crate::geo::no_ground,
                Detail::ELLIPSOID
            )
            .indices
            .is_empty()
        );
        assert!(
            build_fill(
                &[],
                center,
                color,
                &crate::geo::no_ground,
                Detail::ELLIPSOID
            )
            .indices
            .is_empty()
        );
        let identical = vec![point(10.0, 35.0); 5];
        assert!(
            build_fill(
                &[identical],
                center,
                color,
                &crate::geo::no_ground,
                Detail::ELLIPSOID
            )
            .indices
            .is_empty()
        );

        let geometry = PlacemarkGeometry::Polygon {
            outer_ring: vec![point(10.0, 35.0); 4],
            inner_rings: None,
            tessellate: None,
            extrude_height: None,
        };
        let fill = scene_3d_fill(&geometry);
        assert!(fill.is_none_or(|fill| fill.indices.is_empty()));
    }

    #[test]
    fn unfilled_style_produces_no_fill() {
        let mut unfilled = style();
        unfilled.filled = false;
        let geometry = PlacemarkGeometry::circle(point(10.0, 35.0), 150_000.0).expect("circle");
        let (fill, line) = build_meshes(
            &geometry,
            &unfilled,
            false,
            crate::flat::SceneMode::Scene3D,
            &crate::geo::no_ground,
        );
        assert!(fill.is_none_or(|fill| fill.indices.is_empty()));
        assert!(line.is_some(), "the outline is still drawn");
    }

    #[test]
    fn parse_color_handles_css_hex_and_opacity() {
        let color = parse_color("#3366cc", 0.5);
        assert!((color[0] - 0x33 as f32 / 255.0).abs() < 1e-6);
        assert!((color[3] - 0.5).abs() < 1e-6);
    }
}
