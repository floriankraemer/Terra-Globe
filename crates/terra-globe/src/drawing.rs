//! Drawing tools as a pure state machine.
//!
//! The UI forwards clicks and the cursor; this module decides when a shape is
//! complete. Keeping it free of egui makes the commit/cancel semantics testable.

use terra_core::{GeoPoint, PlacemarkGeometry, RectangleBounds};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawTool {
    Marker,
    Line,
    Polygon,
    Circle,
    Rectangle,
}

impl DrawTool {
    pub fn needs_two_clicks(self) -> bool {
        matches!(self, Self::Circle | Self::Rectangle)
    }

    pub fn is_multiclick(self) -> bool {
        matches!(self, Self::Line | Self::Polygon)
    }
}

#[derive(Debug, Clone, Default)]
pub struct DrawingSession {
    tool: Option<DrawTool>,
    vertices: Vec<GeoPoint>,
}

impl DrawingSession {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn tool(&self) -> Option<DrawTool> {
        self.tool
    }

    pub fn is_active(&self) -> bool {
        self.tool.is_some()
    }

    pub fn vertices(&self) -> &[GeoPoint] {
        &self.vertices
    }

    pub fn start(&mut self, tool: DrawTool) {
        self.tool = Some(tool);
        self.vertices.clear();
    }

    pub fn cancel(&mut self) {
        self.tool = None;
        self.vertices.clear();
    }

    /// Removes the last placed vertex; cancels the session when none remain.
    pub fn undo(&mut self) {
        self.vertices.pop();
        if self.vertices.is_empty() {
            self.tool = None;
        }
    }

    /// Registers a click. Returns the finished geometry when the tool completes
    /// on this click (marker, second click of circle/rectangle).
    pub fn click(&mut self, point: GeoPoint) -> Option<PlacemarkGeometry> {
        let tool = self.tool?;
        match tool {
            DrawTool::Marker => {
                let geometry = PlacemarkGeometry::point(point).ok()?;
                self.cancel();
                Some(geometry)
            }
            DrawTool::Circle | DrawTool::Rectangle => {
                self.vertices.push(point);
                if self.vertices.len() < 2 {
                    return None;
                }
                let first = self.vertices[0];
                let second = self.vertices[1];
                self.cancel();
                match tool {
                    DrawTool::Circle => {
                        let radius = terra_core::haversine_meters(&first, &second);
                        PlacemarkGeometry::circle(first, radius).ok()
                    }
                    _ => {
                        let bounds = RectangleBounds::new(
                            first.lat.max(second.lat),
                            first.lat.min(second.lat),
                            first.lon.max(second.lon),
                            first.lon.min(second.lon),
                        )
                        .ok()?;
                        PlacemarkGeometry::rectangle(bounds).ok()
                    }
                }
            }
            DrawTool::Line | DrawTool::Polygon => {
                self.vertices.push(point);
                None
            }
        }
    }

    /// Commits a multi-click shape.
    pub fn finish(&mut self) -> Option<PlacemarkGeometry> {
        let tool = self.tool?;
        let geometry = match tool {
            DrawTool::Line => {
                if self.vertices.len() < 2 {
                    None
                } else {
                    PlacemarkGeometry::line_string(self.vertices.clone(), Some(true), None).ok()
                }
            }
            DrawTool::Polygon => {
                if self.vertices.len() < 3 {
                    None
                } else {
                    PlacemarkGeometry::polygon(self.vertices.clone(), None, Some(true), None).ok()
                }
            }
            _ => None,
        };
        if geometry.is_some() {
            self.cancel();
        }
        geometry
    }

    /// Geometry to show while the user moves the pointer.
    pub fn preview(&self, cursor: Option<GeoPoint>) -> Option<PlacemarkGeometry> {
        let tool = self.tool?;
        let cursor = cursor?;
        match tool {
            DrawTool::Marker => PlacemarkGeometry::point(cursor).ok(),
            DrawTool::Line => {
                let mut path = self.vertices.clone();
                path.push(cursor);
                if path.len() < 2 {
                    return None;
                }
                PlacemarkGeometry::line_string(path, Some(true), None).ok()
            }
            DrawTool::Polygon => {
                let mut ring = self.vertices.clone();
                ring.push(cursor);
                if ring.len() < 3 {
                    return None;
                }
                PlacemarkGeometry::polygon(ring, None, Some(true), None).ok()
            }
            DrawTool::Circle | DrawTool::Rectangle => {
                let first = *self.vertices.first()?;
                self.click_preview(tool, first, cursor)
            }
        }
    }

    fn click_preview(
        &self,
        tool: DrawTool,
        first: GeoPoint,
        cursor: GeoPoint,
    ) -> Option<PlacemarkGeometry> {
        match tool {
            DrawTool::Circle => {
                let radius = terra_core::haversine_meters(&first, &cursor);
                PlacemarkGeometry::circle(first, radius)
                    .ok()
                    .or_else(|| PlacemarkGeometry::circle(first, 1.0).ok())
            }
            _ => {
                let bounds = RectangleBounds::new(
                    first.lat.max(cursor.lat),
                    first.lat.min(cursor.lat),
                    first.lon.max(cursor.lon),
                    first.lon.min(cursor.lon),
                )
                .ok()?;
                PlacemarkGeometry::rectangle(bounds).ok()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(lon: f64, lat: f64) -> GeoPoint {
        GeoPoint::new(lon, lat).expect("valid point")
    }

    #[test]
    fn marker_completes_on_the_first_click() {
        let mut session = DrawingSession::new();
        session.start(DrawTool::Marker);
        let geometry = session.click(point(1.0, 2.0)).expect("completed");
        assert!(matches!(geometry, PlacemarkGeometry::Point { .. }));
        assert!(!session.is_active());
    }

    #[test]
    fn circle_needs_two_clicks_and_uses_great_circle_radius() {
        let mut session = DrawingSession::new();
        session.start(DrawTool::Circle);
        assert!(session.click(point(0.0, 0.0)).is_none());
        assert!(session.is_active());
        let geometry = session.click(point(0.01, 0.0)).expect("completed");
        match geometry {
            PlacemarkGeometry::Circle { radius_meters, .. } => {
                let expected = terra_core::haversine_meters(&point(0.0, 0.0), &point(0.01, 0.0));
                assert!((radius_meters - expected).abs() < 1e-6);
            }
            other => panic!("unexpected geometry {other:?}"),
        }
    }

    #[test]
    fn rectangle_is_normalised_from_any_drag_direction() {
        let mut session = DrawingSession::new();
        session.start(DrawTool::Rectangle);
        session.click(point(10.0, 10.0));
        let geometry = session.click(point(5.0, 5.0)).expect("completed");
        match geometry {
            PlacemarkGeometry::Rectangle {
                north,
                south,
                east,
                west,
            } => {
                assert_eq!((north, south, east, west), (10.0, 5.0, 10.0, 5.0));
            }
            other => panic!("unexpected geometry {other:?}"),
        }
    }

    #[test]
    fn line_and_polygon_commit_on_finish() {
        let mut session = DrawingSession::new();
        session.start(DrawTool::Line);
        session.click(point(0.0, 0.0));
        session.click(point(1.0, 1.0));
        assert!(session.finish().is_some());
        assert!(!session.is_active());

        let mut session = DrawingSession::new();
        session.start(DrawTool::Polygon);
        session.click(point(0.0, 0.0));
        session.click(point(1.0, 0.0));
        assert!(session.finish().is_none(), "two points are not a polygon");
        session.click(point(1.0, 1.0));
        assert!(session.finish().is_some());
    }

    #[test]
    fn preview_tracks_the_cursor_and_cancel_discards() {
        let mut session = DrawingSession::new();
        session.start(DrawTool::Line);
        session.click(point(0.0, 0.0));
        let preview = session.preview(Some(point(1.0, 1.0))).expect("preview");
        assert!(matches!(preview, PlacemarkGeometry::LineString { .. }));
        session.cancel();
        assert!(session.preview(Some(point(1.0, 1.0))).is_none());
    }

    #[test]
    fn undo_removes_vertices_and_then_ends_the_session() {
        let mut session = DrawingSession::new();
        session.start(DrawTool::Polygon);
        session.click(point(0.0, 0.0));
        session.click(point(1.0, 0.0));
        session.undo();
        assert_eq!(session.vertices().len(), 1);
        session.undo();
        assert!(!session.is_active());
    }
}
