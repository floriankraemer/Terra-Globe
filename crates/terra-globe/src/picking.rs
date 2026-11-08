//! CPU picking: screen pixels to the topmost placemark.
//!
//! Analytic tests instead of a GPU id-buffer: deterministic, unit-testable,
//! independent of which tiles have loaded, and no async readback on WebGL2.

use terra_core::{GeoPoint, PlacemarkGeometry};

use crate::camera::{Camera, ScreenPoint};
use crate::flat::{FlatCamera, SURFACE_Z, lon_lat_to_plane};
use crate::primitives::{densify_path, densify_ring};
use crate::scene::Scene;

/// Scale of the app's placemark markers relative to the original design. The
/// app draws markers with it and picking sizes its hit radius from it, so the
/// two cannot drift apart.
pub const MARKER_SCALE: f32 = 1.8;

/// Pixels from a point placemark's anchor that still count as a hit. Sized to
/// the marker scale (the old 14 px times it) so the visible marker edge selects.
pub const POINT_HIT_RADIUS_PX: f64 = 14.0 * MARKER_SCALE as f64;

type Projector<'a> = dyn Fn(&GeoPoint) -> Option<ScreenPoint> + 'a;

#[derive(Debug, Clone, PartialEq)]
pub struct PickHit {
    pub id: String,
    /// Screen position used for the hit (marker anchor or nearest outline point).
    pub screen: ScreenPoint,
    /// Distance in pixels from the pointer to the geometry.
    pub distance_px: f64,
}

/// Picks the closest placemark within its hit tolerance, preferring the nearest
/// to the camera when two overlap.
pub fn pick(
    scene: &Scene,
    camera: &Camera,
    viewport: (u32, u32),
    pixel: (f64, f64),
    ground: &crate::geo::GroundFn<'_>,
) -> Option<PickHit> {
    pick_with(scene, pixel, &|point| {
        camera.project(crate::geo::geo_point_to_ecef(point, ground), viewport)
    })
}

/// Picking against the flat map plane (2D and Columbus).
pub fn pick_flat(
    scene: &Scene,
    camera: &FlatCamera,
    viewport: (u32, u32),
    pixel: (f64, f64),
) -> Option<PickHit> {
    pick_with(scene, pixel, &|point| {
        let plane = lon_lat_to_plane(point);
        camera.project(glam::DVec3::new(plane.x, plane.y, SURFACE_Z), viewport)
    })
}

fn pick_with(scene: &Scene, pixel: (f64, f64), project: &Projector<'_>) -> Option<PickHit> {
    let mut best: Option<(f64, f64, PickHit)> = None;

    for entry in scene.entries.iter().filter(|entry| entry.visible) {
        let width_tolerance = entry.style.outline_width.max(1.0) / 2.0 + 6.0;
        let Some((distance, screen)) =
            hit_distance(&entry.geometry, project, pixel, width_tolerance)
        else {
            continue;
        };
        let depth = (terra_core::geometry_center(&entry.geometry).lat).abs();
        let candidate = PickHit {
            id: entry.id.clone(),
            screen,
            distance_px: distance,
        };
        let better = match &best {
            None => true,
            Some((best_distance, best_depth, _)) => {
                // A clear winner on distance wins; otherwise prefer the nearer
                // geometry (higher latitude is only a tie-breaker to keep this
                // deterministic without another projection).
                distance + 4.0 < *best_distance
                    || ((distance - best_distance).abs() <= 4.0 && depth < *best_depth)
            }
        };
        if better {
            best = Some((distance, depth, candidate));
        }
    }

    best.map(|(_, _, hit)| hit)
}

/// Picks the first entry whose geometry contains/projects near the pixel,
/// ignoring tolerance ranking. Used by tests and simple hover checks.
pub fn pick_id(
    scene: &Scene,
    camera: &Camera,
    viewport: (u32, u32),
    pixel: (f64, f64),
) -> Option<String> {
    pick(scene, camera, viewport, pixel, &crate::geo::no_ground).map(|hit| hit.id)
}

fn hit_distance(
    geometry: &PlacemarkGeometry,
    project: &Projector<'_>,
    pixel: (f64, f64),
    tolerance: f64,
) -> Option<(f64, ScreenPoint)> {
    match geometry {
        PlacemarkGeometry::Point { coordinates }
        | PlacemarkGeometry::Model {
            position: coordinates,
            ..
        } => {
            let screen = project(coordinates)?;
            let distance = ((screen.x - pixel.0).powi(2) + (screen.y - pixel.1).powi(2)).sqrt();
            (distance <= POINT_HIT_RADIUS_PX).then_some((distance, screen))
        }
        PlacemarkGeometry::LineString { path, .. } => {
            hit_polyline(&densify_path(path), project, pixel, tolerance)
        }
        PlacemarkGeometry::Polygon {
            outer_ring,
            inner_rings,
            ..
        } => {
            let ring = densify_ring(outer_ring);
            let holes: Vec<Vec<GeoPoint>> = inner_rings
                .as_deref()
                .unwrap_or(&[])
                .iter()
                .map(|hole| densify_ring(hole))
                .collect();
            hit_area(&ring, &holes, project, pixel, tolerance)
        }
        PlacemarkGeometry::Circle {
            center,
            radius_meters,
        } => {
            let ring = terra_core::circle_to_polygon_ring(center, *radius_meters, 96);
            hit_area(&densify_ring(&ring), &[], project, pixel, tolerance)
        }
        PlacemarkGeometry::Rectangle {
            north,
            south,
            east,
            west,
        } => {
            let ring = rectangle_ring(*north, *south, *east, *west);
            hit_area(&densify_ring(&ring), &[], project, pixel, tolerance)
        }
        PlacemarkGeometry::GroundOverlay { bounds, .. } => {
            let ring = rectangle_ring(bounds.north, bounds.south, bounds.east, bounds.west);
            hit_area(&densify_ring(&ring), &[], project, pixel, tolerance)
        }
    }
}

fn project_ring(ring: &[GeoPoint], project: &Projector<'_>) -> Option<Vec<(f64, f64)>> {
    ring.iter()
        .map(|point| project(point).map(|screen| (screen.x, screen.y)))
        .collect()
}

fn hit_polyline(
    path: &[GeoPoint],
    project: &Projector<'_>,
    pixel: (f64, f64),
    tolerance: f64,
) -> Option<(f64, ScreenPoint)> {
    let projected = project_ring(path, project)?;
    let mut best: Option<(f64, ScreenPoint)> = None;
    for pair in projected.windows(2) {
        let distance = distance_to_segment(pixel, pair[0], pair[1]);
        if distance <= tolerance {
            let screen = nearest_on_segment(pixel, pair[0], pair[1]);
            if best.as_ref().is_none_or(|(current, _)| distance < *current) {
                best = Some((
                    distance,
                    ScreenPoint {
                        x: screen.0,
                        y: screen.1,
                        depth: 0.0,
                    },
                ));
            }
        }
    }
    best
}

fn hit_area(
    ring: &[GeoPoint],
    holes: &[Vec<GeoPoint>],
    project: &Projector<'_>,
    pixel: (f64, f64),
    tolerance: f64,
) -> Option<(f64, ScreenPoint)> {
    let projected = project_ring(ring, project)?;
    let inside = point_in_polygon(pixel, &projected)
        && !holes.iter().any(|hole| {
            project_ring(hole, project)
                .is_some_and(|projected_hole| point_in_polygon(pixel, &projected_hole))
        });
    if inside {
        return Some((
            0.0,
            ScreenPoint {
                x: pixel.0,
                y: pixel.1,
                depth: 0.0,
            },
        ));
    }

    let mut best: Option<(f64, ScreenPoint)> = None;
    for pair in projected
        .windows(2)
        .chain(std::iter::once(std::slice::from_ref(projected.first()?)))
    {
        // The chain above adds the closing edge from the last point to the first.
        let (start, end) = match pair {
            [start, end] => (*start, *end),
            _ => continue,
        };
        let distance = distance_to_segment(pixel, start, end);
        if distance <= tolerance {
            let nearest = nearest_on_segment(pixel, start, end);
            if best.as_ref().is_none_or(|(current, _)| distance < *current) {
                best = Some((
                    distance,
                    ScreenPoint {
                        x: nearest.0,
                        y: nearest.1,
                        depth: 0.0,
                    },
                ));
            }
        }
    }
    best
}

fn distance_to_segment(point: (f64, f64), start: (f64, f64), end: (f64, f64)) -> f64 {
    let nearest = nearest_on_segment(point, start, end);
    ((point.0 - nearest.0).powi(2) + (point.1 - nearest.1).powi(2)).sqrt()
}

pub fn nearest_on_segment(point: (f64, f64), start: (f64, f64), end: (f64, f64)) -> (f64, f64) {
    let dx = end.0 - start.0;
    let dy = end.1 - start.1;
    let length_squared = dx * dx + dy * dy;
    if length_squared <= f64::EPSILON {
        return start;
    }
    let t =
        (((point.0 - start.0) * dx + (point.1 - start.1) * dy) / length_squared).clamp(0.0, 1.0);
    (start.0 + dx * t, start.1 + dy * t)
}

pub fn point_in_polygon(point: (f64, f64), polygon: &[(f64, f64)]) -> bool {
    if polygon.len() < 3 {
        return false;
    }
    let mut inside = false;
    let mut previous = polygon[polygon.len() - 1];
    for &current in polygon {
        let crosses = (current.1 > point.1) != (previous.1 > point.1)
            && point.0
                < (previous.0 - current.0) * (point.1 - current.1) / (previous.1 - current.1)
                    + current.0;
        if crosses {
            inside = !inside;
        }
        previous = current;
    }
    inside
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::Camera;
    use crate::geo::no_ground;
    use terra_core::{NewStyle, PlacemarkGeometry, create_style};

    /// Raw (unclamped) position, to show where a floating point would be.
    fn ecef_of(point: &GeoPoint) -> glam::DVec3 {
        crate::geo::geodetic_to_ecef(crate::geo::Geodetic::from_degrees(
            point.lon,
            point.lat,
            point.altitude.unwrap_or(0.0),
        ))
    }

    fn style() -> terra_core::Style {
        create_style(NewStyle {
            outline_color: "#ffffff".into(),
            outline_width: 2.0,
            fill_color: "#ffffff".into(),
            fill_opacity: 0.5,
            outline_enabled: None,
            filled: Some(true),
            icon_url: None,
            icon_scale: None,
            marker_icon: None,
            label_color: None,
        })
        .expect("style")
    }

    fn core_point(geodetic: crate::geo::Geodetic) -> GeoPoint {
        GeoPoint::new(geodetic.lon_deg(), geodetic.lat_deg()).expect("valid point")
    }

    fn scene_with(geometry: PlacemarkGeometry) -> Scene {
        Scene {
            entries: vec![crate::scene::SceneEntry {
                id: "p1".into(),
                geometry,
                style: style(),
                visible: true,
                selected: false,
            }],
            ..Default::default()
        }
    }

    #[test]
    fn picks_a_marker_at_its_projected_position() {
        let camera = Camera::default();
        let point = core_point(camera.target_geodetic());
        let geometry = PlacemarkGeometry::point(point).expect("point");
        let scene = scene_with(geometry);
        let screen = camera
            .project(ecef_of(&point), (1280, 800))
            .expect("projected");
        let hit = pick(
            &scene,
            &camera,
            (1280, 800),
            (screen.x + 3.0, screen.y + 2.0),
            &no_ground,
        );
        assert_eq!(hit.map(|hit| hit.id), Some("p1".to_string()));
        assert!(
            pick(
                &scene,
                &camera,
                (1280, 800),
                (screen.x + 100.0, screen.y),
                &no_ground
            )
            .is_none()
        );
    }

    #[test]
    fn picks_a_clamped_high_altitude_point_where_it_is_drawn() {
        let camera = Camera::default();
        let mut point = core_point(camera.target_geodetic());
        point.altitude = Some(50_000.0);
        let scene = scene_with(PlacemarkGeometry::point(point).expect("point"));
        let drawn = camera
            .project(
                crate::geo::geo_point_to_ecef(&point, &no_ground),
                (1280, 800),
            )
            .expect("projected");
        let hit = pick(&scene, &camera, (1280, 800), (drawn.x, drawn.y), &no_ground);
        assert_eq!(hit.map(|hit| hit.id), Some("p1".to_string()));
        let floating = camera
            .project(ecef_of(&point), (1280, 800))
            .expect("floating");
        assert!(
            pick(
                &scene,
                &camera,
                (1280, 800),
                (floating.x, floating.y - 80.0),
                &no_ground
            )
            .is_none()
        );
    }

    #[test]
    fn picks_a_polygon_anywhere_inside_it() {
        let camera = Camera::default();
        let center = core_point(camera.target_geodetic());
        let geometry = PlacemarkGeometry::circle(center, 200_000.0).expect("circle");
        let scene = scene_with(geometry);
        let screen = camera
            .project(ecef_of(&center), (1280, 800))
            .expect("projected");
        assert!(
            pick(
                &scene,
                &camera,
                (1280, 800),
                (screen.x, screen.y),
                &no_ground
            )
            .is_some()
        );
        assert!(pick(&scene, &camera, (1280, 800), (10.0, 10.0), &no_ground).is_none());
    }

    #[test]
    fn picks_a_line_near_its_projection() {
        let camera = Camera::default();
        let center = core_point(camera.target_geodetic());
        let east = GeoPoint {
            lon: center.lon + 0.5,
            lat: center.lat,
            altitude: None,
            altitude_mode: None,
        };
        let geometry =
            PlacemarkGeometry::line_string(vec![center, east], None, None).expect("line");
        let scene = scene_with(geometry);
        let start = camera
            .project(ecef_of(&center), (1280, 800))
            .expect("start");
        let end = camera.project(ecef_of(&east), (1280, 800)).expect("end");
        let middle = ((start.x + end.x) / 2.0, (start.y + end.y) / 2.0);
        assert!(
            pick(&scene, &camera, (1280, 800), middle, &no_ground).is_some(),
            "line should be pickable along its length"
        );
    }

    #[test]
    fn point_in_polygon_handles_concave_shapes() {
        let polygon = [(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (2.0, 1.0), (0.0, 4.0)];
        assert!(point_in_polygon((1.0, 1.0), &polygon));
        assert!(!point_in_polygon((3.5, 3.5), &polygon));
        assert!(!point_in_polygon((-1.0, -1.0), &polygon));
    }

    #[test]
    fn nearest_point_clamps_to_the_segment() {
        assert_eq!(
            nearest_on_segment((5.0, 5.0), (0.0, 0.0), (10.0, 0.0)),
            (5.0, 0.0)
        );
        assert_eq!(
            nearest_on_segment((-5.0, 0.0), (0.0, 0.0), (10.0, 0.0)),
            (0.0, 0.0)
        );
        assert_eq!(
            nearest_on_segment((15.0, 0.0), (0.0, 0.0), (10.0, 0.0)),
            (10.0, 0.0)
        );
    }

    #[test]
    fn picks_a_marker_where_terrain_lifts_it() {
        let camera = Camera {
            distance: 20_000.0,
            pitch: 40f64.to_radians(),
            ..Camera::default()
        };
        let point = core_point(camera.target_geodetic());
        let scene = scene_with(PlacemarkGeometry::point(point).expect("point"));
        let mountain = |_: f64, _: f64| Some(3_000.0);
        let lifted = camera
            .project(
                crate::geo::geo_point_to_ecef(&point, &mountain),
                (1280, 800),
            )
            .expect("projected");
        let hit = pick(
            &scene,
            &camera,
            (1280, 800),
            (lifted.x, lifted.y),
            &mountain,
        );
        assert_eq!(hit.map(|hit| hit.id), Some("p1".to_string()));
        // The same pixel misses the marker when the terrain is not known.
        assert!(
            pick(
                &scene,
                &camera,
                (1280, 800),
                (lifted.x, lifted.y),
                &no_ground
            )
            .is_none()
        );
    }
}
