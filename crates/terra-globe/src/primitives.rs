//! Converts placemark geometry into GPU meshes: filled polygons and
//! screen-space-width polylines.
//!
//! Vertices are stored relative to the geometry centre (`center_ecef`) and the
//! renderer supplies the eye-relative translation, so nothing large reaches the
//! GPU in `f32`.

use glam::{DVec3, Vec3};
use terra_core::{GeoPoint, PlacemarkGeometry, Style};

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

/// Overlay geometry is lifted along the ellipsoid normal so it wins the depth
/// test against the globe's own tessellated surface (whose tiles are coarse
/// chords that can sit above the true ellipsoid). Three metres is invisible at
/// globe scale and small enough not to matter at street level.
const SURFACE_LIFT_METERS: f64 = 3.0;

pub fn build_meshes(
    geometry: &PlacemarkGeometry,
    style: &Style,
    selected: bool,
    mode: SceneMode,
) -> (Option<FillMesh>, Option<LineMesh>) {
    if mode.is_flat() {
        return build_flat_meshes(geometry, style, selected);
    }
    let center = crate::geo::geo_point_to_ecef(&terra_core::geometry_center(geometry));
    let (outline_color, outline_width) = outline_style(style, selected);
    let fill_color = fill_color(style);

    let (rings, closed) = match geometry {
        PlacemarkGeometry::Polygon {
            outer_ring,
            inner_rings,
            ..
        } => {
            let mut rings = vec![densify_ring(outer_ring)];
            if let Some(inner) = inner_rings {
                rings.extend(inner.iter().map(|ring| densify_ring(ring)));
            }
            (rings, true)
        }
        PlacemarkGeometry::Rectangle {
            north,
            south,
            east,
            west,
        } => (
            vec![densify_ring(&rectangle_ring(*north, *south, *east, *west))],
            true,
        ),
        PlacemarkGeometry::Circle {
            center: circle_center,
            radius_meters,
        } => (
            vec![densify_ring(&terra_core::circle_to_polygon_ring(
                circle_center,
                *radius_meters,
                96,
            ))],
            true,
        ),
        PlacemarkGeometry::GroundOverlay { bounds, .. } => (
            vec![densify_ring(&rectangle_ring(
                bounds.north,
                bounds.south,
                bounds.east,
                bounds.west,
            ))],
            true,
        ),
        PlacemarkGeometry::LineString { path, .. } => (vec![densify_path(path)], false),
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
            .map(|ring| ring.iter().map(|point| ecef_local(point, center)).collect())
            .collect();
        for ring in &points {
            append_polyline(&mut mesh, ring, width as f32, outline_color, closed);
        }
        mesh
    });

    let fill = if closed && style.filled {
        Some(build_fill(&rings, center, fill_color))
    } else if closed && matches!(geometry, PlacemarkGeometry::GroundOverlay { .. }) {
        // Ground overlays always show their extent even without a style fill.
        Some(build_fill(&rings, center, [1.0, 1.0, 1.0, 0.18]))
    } else {
        None
    };

    (fill, line)
}

/// Local ECEF position with the surface lift applied.
fn ecef_local(point: &GeoPoint, center: DVec3) -> Vec3 {
    let local = (geodetic_to_ecef(crate::geo::Geodetic::from_degrees(
        point.lon,
        point.lat,
        point.altitude.unwrap_or(0.0),
    )) - center)
        .as_vec3();
    lift(local, center)
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

/// Moves a local vertex outward along the ellipsoid normal.
fn lift(local: Vec3, center: DVec3) -> Vec3 {
    let world = center + local.as_dvec3();
    let normal = crate::geo::ellipsoid_normal(world).as_vec3();
    local + normal * SURFACE_LIFT_METERS as f32
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
    if ring.len() < 2 {
        return ring.to_vec();
    }
    let mut out = Vec::with_capacity(ring.len());
    for pair in ring.windows(2) {
        append_segment(&mut out, &pair[0], &pair[1]);
    }
    if let (Some(first), Some(last)) = (ring.first(), ring.last()) {
        if first.lon != last.lon || first.lat != last.lat {
            append_segment(&mut out, last, first);
        }
    }
    out.push(*ring.last().expect("non-empty ring"));
    out
}

pub fn densify_path(path: &[GeoPoint]) -> Vec<GeoPoint> {
    if path.len() < 2 {
        return path.to_vec();
    }
    let mut out = Vec::with_capacity(path.len());
    for pair in path.windows(2) {
        append_segment(&mut out, &pair[0], &pair[1]);
    }
    if let Some(last) = path.last() {
        out.push(*last);
    }
    out
}

fn append_segment(out: &mut Vec<GeoPoint>, a: &GeoPoint, b: &GeoPoint) {
    let distance = terra_core::haversine_meters(a, b);
    let steps = ((distance / MAX_EDGE_METERS).ceil() as usize).max(1);
    out.push(*a);
    for step in 1..steps {
        out.push(interpolate(a, b, step as f64 / steps as f64));
    }
}

/// Great-circle interpolation so antimeridian crossings take the short way.
fn interpolate(a: &GeoPoint, b: &GeoPoint, t: f64) -> GeoPoint {
    let start = geodetic_to_ecef(crate::geo::Geodetic::from_degrees(
        a.lon,
        a.lat,
        a.altitude.unwrap_or(0.0),
    ))
    .normalize();
    let end = geodetic_to_ecef(crate::geo::Geodetic::from_degrees(
        b.lon,
        b.lat,
        b.altitude.unwrap_or(0.0),
    ))
    .normalize();
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

fn build_fill(rings: &[Vec<GeoPoint>], center: DVec3, color: [f32; 4]) -> FillMesh {
    let (east, north, _) = enu_axes(center);

    let mut flat: Vec<f64> = Vec::new();
    let mut holes: Vec<usize> = Vec::new();
    let mut world: Vec<Vec3> = Vec::new();

    for (ring_index, ring) in rings.iter().enumerate() {
        if ring.len() < 3 {
            continue;
        }
        if ring_index > 0 {
            holes.push(flat.len() / 2);
        }
        for point in ring {
            let ecef = geodetic_to_ecef(crate::geo::Geodetic::from_degrees(
                point.lon,
                point.lat,
                point.altitude.unwrap_or(0.0),
            ));
            let local = ecef - center;
            flat.push(local.dot(east));
            flat.push(local.dot(north));
            world.push(local.as_vec3());
        }
    }

    if world.is_empty() {
        return FillMesh::default();
    }

    let Ok(indices) = earcutr::earcut(&flat, &holes, 2) else {
        return FillMesh::default();
    };

    FillMesh {
        vertices: world
            .into_iter()
            .map(|position| FillVertex {
                position: lift(position, center).to_array(),
                color,
            })
            .collect(),
        indices: indices.into_iter().map(|index| index as u32).collect(),
        center,
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
        let (fill, line) =
            build_meshes(&geometry, &style(), false, crate::flat::SceneMode::Scene3D);
        let fill = fill.expect("filled polygon");
        assert!(!fill.vertices.is_empty());
        assert_eq!(fill.indices.len() % 3, 0);
        let line = line.expect("outline");
        assert!(!line.vertices.is_empty());
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
        let (fill, _) = build_meshes(&geometry, &style(), false, crate::flat::SceneMode::Scene3D);
        let fill = fill.expect("fill");
        assert!(fill.vertices.len() >= 8);
        assert!(fill.indices.len() >= 18);
    }

    #[test]
    fn unfilled_shapes_get_only_an_outline() {
        let mut style = style();
        style.filled = false;
        let geometry = PlacemarkGeometry::circle(point(0.0, 0.0), 500.0).expect("circle");
        let (fill, line) = build_meshes(&geometry, &style, false, crate::flat::SceneMode::Scene3D);
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
        let (_, line) = build_meshes(&geometry, &style, false, crate::flat::SceneMode::Scene3D);
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
        let (_, normal) = build_meshes(&geometry, &style(), false, crate::flat::SceneMode::Scene3D);
        let (_, selected) =
            build_meshes(&geometry, &style(), true, crate::flat::SceneMode::Scene3D);
        let normal_width = normal.expect("line").vertices[0].width;
        let selected_width = selected.expect("line").vertices[0].width;
        assert!(selected_width > normal_width);
    }

    #[test]
    fn parse_color_handles_css_hex_and_opacity() {
        let color = parse_color("#3366cc", 0.5);
        assert!((color[0] - 0x33 as f32 / 255.0).abs() < 1e-6);
        assert!((color[3] - 0.5).abs() < 1e-6);
    }
}
