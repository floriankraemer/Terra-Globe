//! Reusable IDE-style tree: a pure row model, keyboard navigation and a painted row widget.
//!
//! The Places tree and the Layers section both draw their rows with `tree_row`.

use eframe::egui::{
    self, Color32, Id, Pos2, Rect, Response, Sense, Shape, Stroke, StrokeKind, Ui, pos2, vec2,
};
use terra_core::PlacemarkGeometry;

use crate::state::Library;
use crate::theme;

pub const ROW_HEIGHT: f32 = 27.0;
pub const INDENT: f32 = 16.0;
const PAD_X: f32 = 4.0;
const ICON_SIZE: f32 = 16.0;
const EYE_WIDTH: f32 = 22.0;
const HIGHLIGHT_RADIUS: u8 = 6;
const FOLDER_COLOR: Color32 = Color32::from_rgb(156, 168, 190);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKind {
    Folder,
    Placemark,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisibleRow {
    pub id: String,
    pub kind: RowKind,
    pub depth: usize,
    pub has_children: bool,
    pub open: bool,
    pub has_sibling_below: bool,
    /// Indexed by ancestor depth: whether that ancestor has a sibling below it.
    pub ancestors_have_sibling_below: Vec<bool>,
}

/// Rows in display order. `query` must already be lowercase.
pub fn visible_rows(library: &Library, query: &str) -> Vec<VisibleRow> {
    let mut rows = Vec::new();
    push_level(library, query, None, &mut Vec::new(), &mut rows);
    rows
}

fn push_level(
    library: &Library,
    query: &str,
    parent: Option<&str>,
    ancestors: &mut Vec<bool>,
    rows: &mut Vec<VisibleRow>,
) {
    let entries = level_entries(library, query, parent);
    let last = entries.len();
    for (index, (id, kind)) in entries.into_iter().enumerate() {
        let has_sibling_below = index + 1 < last;
        let mut row = VisibleRow {
            id: id.clone(),
            kind,
            depth: ancestors.len(),
            has_children: false,
            open: false,
            has_sibling_below,
            ancestors_have_sibling_below: ancestors.clone(),
        };
        if kind == RowKind::Folder {
            row.has_children = !level_entries(library, query, Some(&id)).is_empty();
            row.open = library
                .folder(&id)
                .is_some_and(|folder| folder.open.unwrap_or(true));
        }
        let descend = row.has_children && row.open;
        rows.push(row);
        if descend {
            ancestors.push(has_sibling_below);
            push_level(library, query, Some(&id), ancestors, rows);
            ancestors.pop();
        }
    }
}

fn level_entries(library: &Library, query: &str, parent: Option<&str>) -> Vec<(String, RowKind)> {
    let folders = library
        .child_folders(parent)
        .into_iter()
        .filter(|folder| {
            query.is_empty()
                || folder.name.to_lowercase().contains(query)
                || subtree_matches(library, &folder.id, query)
        })
        .map(|folder| (folder.id.clone(), RowKind::Folder));
    let placemarks = library
        .placemarks_in(parent)
        .into_iter()
        .filter(|placemark| query.is_empty() || placemark.name.to_lowercase().contains(query))
        .map(|placemark| (placemark.id.clone(), RowKind::Placemark));
    folders.chain(placemarks).collect()
}

fn subtree_matches(library: &Library, folder_id: &str, query: &str) -> bool {
    library
        .placemarks_in(Some(folder_id))
        .iter()
        .any(|placemark| placemark.name.to_lowercase().contains(query))
        || library.child_folders(Some(folder_id)).iter().any(|child| {
            child.name.to_lowercase().contains(query) || subtree_matches(library, &child.id, query)
        })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavKey {
    Up,
    Down,
    Left,
    Right,
    Enter,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavAction {
    None,
    Select(String),
    Collapse(String),
    Expand(String),
    FlyTo(String),
}

pub fn next_selection(rows: &[VisibleRow], current: Option<&str>, key: NavKey) -> NavAction {
    let Some(index) = current.and_then(|id| rows.iter().position(|row| row.id == id)) else {
        return match (key, rows.first()) {
            (NavKey::Down, Some(first)) => NavAction::Select(first.id.clone()),
            _ => NavAction::None,
        };
    };
    let Some(row) = rows.get(index) else {
        return NavAction::None;
    };
    let select = |target: Option<&VisibleRow>| {
        target.map_or(NavAction::None, |row| NavAction::Select(row.id.clone()))
    };
    match key {
        NavKey::Down => select(rows.get(index + 1)),
        NavKey::Up => select(index.checked_sub(1).and_then(|i| rows.get(i))),
        NavKey::Enter => NavAction::FlyTo(row.id.clone()),
        NavKey::Left if row.has_children && row.open => NavAction::Collapse(row.id.clone()),
        NavKey::Left => select(
            rows.get(..index)
                .and_then(|before| before.iter().rev().find(|r| r.depth + 1 == row.depth)),
        ),
        NavKey::Right if row.has_children && !row.open => NavAction::Expand(row.id.clone()),
        NavKey::Right if row.has_children => select(rows.get(index + 1)),
        NavKey::Right => NavAction::None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconKind {
    FolderClosed,
    FolderOpen,
    Point,
    Line,
    Polygon,
    Circle,
    Rectangle,
    Image,
    Model,
    Layer,
}

/// The widget at the right edge of a row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowControl {
    /// Visibility toggle, drawn as an eye.
    Eye,
    /// Exclusive choice, drawn as a radio button; `RowSpec::visible` is "chosen".
    Radio,
    None,
}

pub fn icon_for_geometry(geometry: &PlacemarkGeometry) -> IconKind {
    match geometry {
        PlacemarkGeometry::Point { .. } => IconKind::Point,
        PlacemarkGeometry::LineString { .. } => IconKind::Line,
        PlacemarkGeometry::Polygon { .. } => IconKind::Polygon,
        PlacemarkGeometry::Circle { .. } => IconKind::Circle,
        PlacemarkGeometry::Rectangle { .. } => IconKind::Rectangle,
        PlacemarkGeometry::GroundOverlay { .. } => IconKind::Image,
        PlacemarkGeometry::Model { .. } => IconKind::Model,
    }
}

pub struct RowSpec<'a> {
    pub id: Id,
    pub depth: usize,
    pub ancestors_have_sibling_below: &'a [bool],
    pub has_sibling_below: bool,
    pub has_children: bool,
    pub open: bool,
    pub icon: IconKind,
    pub tint: Option<Color32>,
    pub label: &'a str,
    pub selected: bool,
    /// Hidden by itself or by an ancestor: the label is dimmed.
    pub dimmed: bool,
    /// The row's own visibility flag, shown by the eye.
    pub visible: bool,
    pub control: RowControl,
    pub eye_label: &'a str,
    pub chevron_label: &'a str,
    /// Highlight the row as a drop target while something is dragged over it.
    pub accepts_drop: bool,
}

pub struct RowResponse {
    pub row: Response,
    pub chevron_clicked: bool,
    pub eye_clicked: bool,
}

pub fn tree_row(ui: &mut Ui, spec: &RowSpec<'_>) -> RowResponse {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(width, ROW_HEIGHT), Sense::hover());
    // Rows are not focusable: egui would otherwise move focus on arrow keys,
    // which the tree handles itself.
    let row = ui.interact(rect, spec.id, Sense::CLICK | Sense::DRAG);
    row.widget_info(|| {
        egui::WidgetInfo::selected(egui::Role::TreeItem, true, spec.selected, spec.label)
    });

    let item_cell = cell_rect(rect, spec.depth);
    let chevron = spec.has_children.then(|| {
        let response = ui.interact(item_cell, spec.id.with("chevron"), Sense::CLICK);
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::Role::Button, true, spec.chevron_label)
        });
        response
    });
    let eye_rect = Rect::from_min_max(
        pos2(rect.right() - PAD_X - EYE_WIDTH, rect.top()),
        pos2(rect.right() - PAD_X, rect.bottom()),
    );
    let eye = ui.interact(eye_rect, spec.id.with("eye"), Sense::CLICK);
    let role = if spec.control == RowControl::Radio {
        egui::Role::RadioButton
    } else {
        egui::Role::CheckBox
    };
    if spec.control != RowControl::None {
        eye.widget_info(|| egui::WidgetInfo::selected(role, true, spec.visible, spec.eye_label));
    }

    if ui.is_rect_visible(rect) {
        paint_row(ui, spec, rect, &row, &eye);
    }
    if row.dragged() {
        paint_drag_ghost(ui, spec.label);
    }

    RowResponse {
        chevron_clicked: chevron.is_some_and(|response| response.clicked()),
        eye_clicked: spec.control != RowControl::None && eye.clicked(),
        row,
    }
}

fn cell_rect(row: Rect, cell: usize) -> Rect {
    let left = row.left() + cell as f32 * INDENT;
    Rect::from_min_max(pos2(left, row.top()), pos2(left + INDENT, row.bottom()))
}

fn crisp(value: f32) -> f32 {
    value.floor() + 0.5
}

fn paint_row(ui: &Ui, spec: &RowSpec<'_>, rect: Rect, row: &Response, eye: &Response) {
    let painter = ui.painter();
    let content_left = rect.left() + (spec.depth + 1) as f32 * INDENT;
    let highlight = Rect::from_min_max(
        pos2(content_left, rect.top()),
        pos2(rect.right() - PAD_X, rect.bottom()),
    );
    let drag_active = egui::DragAndDrop::has_any_payload(ui.ctx());
    let hovered = row.hovered() || eye.hovered() || (drag_active && row.contains_pointer());
    let fill = if spec.selected {
        Some(theme::ROW_SELECTED)
    } else if hovered {
        Some(theme::ROW_HOVER)
    } else {
        None
    };
    if let Some(fill) = fill {
        painter.rect_filled(highlight, HIGHLIGHT_RADIUS, fill);
    }
    if spec.accepts_drop && drag_active && row.contains_pointer() {
        painter.rect_stroke(
            highlight,
            HIGHLIGHT_RADIUS,
            Stroke::new(1.0, theme::ACCENT),
            StrokeKind::Inside,
        );
    }

    paint_guides(painter, spec, rect);

    let centre_y = rect.center().y;
    let icon_rect = Rect::from_min_size(
        pos2(content_left + PAD_X, centre_y - ICON_SIZE / 2.0),
        vec2(ICON_SIZE, ICON_SIZE),
    );
    let default_color = match spec.icon {
        IconKind::FolderClosed | IconKind::FolderOpen => FOLDER_COLOR,
        _ => theme::ACCENT,
    };
    let mut icon_color = spec.tint.unwrap_or(default_color);
    if spec.dimmed {
        icon_color = icon_color.gamma_multiply(0.5);
    }
    paint_icon(painter, spec.icon, icon_rect, icon_color);

    let show_eye = match spec.control {
        RowControl::Eye => hovered || spec.selected || !spec.visible,
        RowControl::Radio => true,
        RowControl::None => false,
    };
    let text_left = icon_rect.right() + 6.0;
    let text_right = if show_eye {
        eye_rect_left(rect) - 2.0
    } else {
        highlight.right() - PAD_X
    };
    let text_color = if spec.dimmed {
        theme::TEXT_MUTED
    } else {
        theme::TEXT
    };
    let mut job = egui::text::LayoutJob::simple_singleline(
        spec.label.to_owned(),
        egui::TextStyle::Body.resolve(ui.style()),
        text_color,
    );
    job.wrap.max_width = (text_right - text_left).max(0.0);
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    let galley = painter.layout_job(job);
    let text_pos = pos2(text_left, centre_y - galley.size().y / 2.0);
    painter.galley(text_pos, galley, text_color);

    if show_eye {
        let colour = if spec.visible {
            theme::TEXT_MUTED
        } else {
            theme::TEXT_MUTED.gamma_multiply(0.6)
        };
        if spec.control == RowControl::Radio {
            paint_radio(painter, eye.rect.center(), spec.visible);
        } else {
            paint_eye(painter, eye.rect.center(), spec.visible, colour);
        }
    }
}

fn eye_rect_left(row: Rect) -> f32 {
    row.right() - PAD_X - EYE_WIDTH
}

fn paint_guides(painter: &egui::Painter, spec: &RowSpec<'_>, rect: Rect) {
    let guide = Stroke::new(1.0, theme::TEXT_MUTED.gamma_multiply(96.0 / 255.0));
    let half = (ROW_HEIGHT / 5.0).floor().max(2.0);
    let centre_y = crisp(rect.center().y);
    for cell in 0..=spec.depth {
        let cell_rect = cell_rect(rect, cell);
        let centre_x = crisp(cell_rect.center().x);
        let parent_x = centre_x - INDENT;
        let has_parent_column = parent_x >= rect.left();
        let is_item = cell == spec.depth;
        if is_item {
            if spec.has_children && spec.open {
                painter.line_segment(
                    [
                        pos2(centre_x, centre_y + half + 2.0),
                        pos2(centre_x, rect.bottom()),
                    ],
                    guide,
                );
            }
            if has_parent_column {
                let bottom = if spec.has_sibling_below {
                    rect.bottom()
                } else {
                    centre_y
                };
                let right = if spec.has_children {
                    centre_x - half - 2.0
                } else {
                    cell_rect.right()
                };
                painter.line_segment([pos2(parent_x, rect.top()), pos2(parent_x, bottom)], guide);
                painter.line_segment([pos2(parent_x, centre_y), pos2(right, centre_y)], guide);
            }
            if spec.has_children {
                paint_chevron(painter, pos2(centre_x, centre_y), spec.open, half);
            }
        } else if has_parent_column
            && spec
                .ancestors_have_sibling_below
                .get(cell)
                .copied()
                .unwrap_or(false)
        {
            painter.line_segment(
                [pos2(parent_x, rect.top()), pos2(parent_x, rect.bottom())],
                guide,
            );
        }
    }
}

/// Left offset of a row's icon at `depth`, for inline content that must line up with it.
pub fn content_indent(depth: usize) -> f32 {
    (depth + 1) as f32 * INDENT + PAD_X
}

/// Continues the parent guide line of a row at `depth` through extra content
/// shown below it (e.g. an inline slider), so the tree stays connected.
pub fn paint_guide_through(painter: &egui::Painter, rect: Rect, depth: usize) {
    if depth == 0 {
        return;
    }
    let guide = Stroke::new(1.0, theme::TEXT_MUTED.gamma_multiply(96.0 / 255.0));
    let x = crisp(cell_rect(rect, depth).center().x) - INDENT;
    painter.line_segment([pos2(x, rect.top()), pos2(x, rect.bottom())], guide);
}

pub fn paint_chevron(painter: &egui::Painter, centre: Pos2, open: bool, half: f32) {
    let points = if open {
        vec![
            centre + vec2(-half, -half / 2.0),
            centre + vec2(0.0, half / 2.0),
            centre + vec2(half, -half / 2.0),
        ]
    } else {
        vec![
            centre + vec2(-half / 2.0, -half),
            centre + vec2(half / 2.0, 0.0),
            centre + vec2(-half / 2.0, half),
        ]
    };
    painter.add(Shape::line(points, Stroke::new(1.5, theme::TEXT_MUTED)));
}

fn paint_eye(painter: &egui::Painter, centre: Pos2, visible: bool, colour: Color32) {
    let stroke = Stroke::new(1.3, colour);
    let curve = |sign: f32| -> Vec<Pos2> {
        (-6..=6)
            .map(|step| {
                let t = step as f32 / 6.0;
                pos2(centre.x + 7.0 * t, centre.y + sign * 4.5 * (1.0 - t * t))
            })
            .collect()
    };
    painter.add(Shape::line(curve(-1.0), stroke));
    painter.add(Shape::line(curve(1.0), stroke));
    painter.circle_filled(centre, 2.0, colour);
    if !visible {
        painter.line_segment(
            [centre + vec2(-6.0, 6.0), centre + vec2(6.0, -6.0)],
            Stroke::new(1.5, colour),
        );
    }
}

fn paint_radio(painter: &egui::Painter, centre: Pos2, chosen: bool) {
    let colour = if chosen {
        theme::ACCENT
    } else {
        theme::TEXT_MUTED
    };
    painter.circle_stroke(centre, 6.0, Stroke::new(1.3, colour));
    if chosen {
        painter.circle_filled(centre, 3.0, colour);
    }
}

fn paint_drag_ghost(ui: &Ui, label: &str) {
    let Some(pointer) = ui.ctx().pointer_interact_pos() else {
        return;
    };
    let painter = ui.ctx().layer_painter(egui::LayerId::new(
        egui::Order::Tooltip,
        Id::unique("tree-drag-ghost"),
    ));
    let galley = painter.layout_no_wrap(
        label.to_owned(),
        egui::TextStyle::Body.resolve(ui.style()),
        theme::TEXT,
    );
    let rect =
        Rect::from_min_size(pointer + vec2(12.0, 8.0), galley.size()).expand2(vec2(8.0, 4.0));
    painter.rect_filled(rect, HIGHLIGHT_RADIUS, theme::ROW_SELECTED);
    painter.galley(rect.min + vec2(8.0, 4.0), galley, theme::TEXT);
}

/// Paints one 16 px glyph inside `rect`; geometry is in a 16x16 design grid.
pub fn paint_icon(painter: &egui::Painter, kind: IconKind, rect: Rect, colour: Color32) {
    let at = |x: f32, y: f32| rect.min + vec2(x, y);
    let fill = colour.gamma_multiply(0.35);
    let stroke = Stroke::new(1.4, colour);
    match kind {
        IconKind::FolderClosed => {
            painter.rect_filled(Rect::from_two_pos(at(1.0, 3.0), at(7.0, 6.0)), 1.0, colour);
            painter.rect_filled(
                Rect::from_two_pos(at(1.0, 5.0), at(15.0, 13.0)),
                1.5,
                colour,
            );
        }
        IconKind::FolderOpen => {
            painter.rect_stroke(
                Rect::from_two_pos(at(1.5, 3.5), at(14.5, 12.5)),
                1.5,
                Stroke::new(1.2, colour),
                StrokeKind::Inside,
            );
            painter.add(Shape::convex_polygon(
                vec![at(3.5, 7.0), at(15.5, 7.0), at(13.0, 13.0), at(1.0, 13.0)],
                colour,
                Stroke::NONE,
            ));
        }
        IconKind::Point => {
            painter.add(Shape::convex_polygon(
                vec![
                    at(8.0, 14.5),
                    at(3.5, 7.0),
                    at(4.5, 3.5),
                    at(11.5, 3.5),
                    at(12.5, 7.0),
                ],
                colour,
                Stroke::NONE,
            ));
            painter.circle_filled(at(8.0, 6.5), 2.5, colour);
            painter.circle_filled(at(8.0, 6.5), 1.2, colour.gamma_multiply(0.3));
        }
        IconKind::Line => {
            painter.add(Shape::line(
                vec![at(2.0, 12.5), at(6.0, 6.0), at(10.0, 10.0), at(14.0, 3.5)],
                Stroke::new(1.8, colour),
            ));
            painter.circle_filled(at(2.0, 12.5), 1.8, colour);
            painter.circle_filled(at(14.0, 3.5), 1.8, colour);
        }
        IconKind::Polygon => {
            painter.add(Shape::convex_polygon(
                vec![
                    at(8.0, 2.0),
                    at(14.0, 6.5),
                    at(11.5, 13.5),
                    at(4.5, 13.5),
                    at(2.0, 6.5),
                ],
                fill,
                stroke,
            ));
        }
        IconKind::Circle => {
            painter.circle(at(8.0, 8.0), 6.0, fill, stroke);
        }
        IconKind::Rectangle => {
            painter.rect(
                Rect::from_two_pos(at(2.0, 3.5), at(14.0, 12.5)),
                1.0,
                fill,
                stroke,
                StrokeKind::Inside,
            );
        }
        IconKind::Image => {
            painter.rect_stroke(
                Rect::from_two_pos(at(1.5, 2.5), at(14.5, 13.5)),
                1.5,
                stroke,
                StrokeKind::Inside,
            );
            painter.circle_filled(at(5.5, 6.0), 1.5, colour);
            painter.add(Shape::line(
                vec![
                    at(2.5, 12.0),
                    at(6.5, 8.5),
                    at(9.0, 10.5),
                    at(11.0, 8.5),
                    at(14.0, 11.5),
                ],
                Stroke::new(1.2, colour),
            ));
        }
        IconKind::Layer => {
            let rhombus = |top: f32| {
                vec![
                    at(8.0, top),
                    at(14.5, top + 3.0),
                    at(8.0, top + 6.0),
                    at(1.5, top + 3.0),
                ]
            };
            painter.add(Shape::convex_polygon(rhombus(7.5), fill, stroke));
            painter.add(Shape::convex_polygon(rhombus(2.5), fill, stroke));
        }
        IconKind::Model => {
            let hexagon = vec![
                at(8.0, 1.5),
                at(14.0, 4.75),
                at(14.0, 11.25),
                at(8.0, 14.5),
                at(2.0, 11.25),
                at(2.0, 4.75),
            ];
            painter.add(Shape::convex_polygon(hexagon, fill, stroke));
            let hub = at(8.0, 8.0);
            for corner in [at(2.0, 4.75), at(14.0, 4.75), at(8.0, 14.5)] {
                painter.line_segment([hub, corner], Stroke::new(1.0, colour));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use terra_core::{GeoPoint, Patch, PlacemarkPatch};
    use terra_storage::InMemoryPlacesRepository;

    use crate::state::StateStore;

    fn point() -> PlacemarkGeometry {
        PlacemarkGeometry::Point {
            coordinates: GeoPoint::new(1.0, 2.0).expect("point"),
        }
    }

    fn store() -> StateStore {
        StateStore::load(Arc::new(InMemoryPlacesRepository::new())).expect("store")
    }

    fn add_placemark(store: &mut StateStore, name: &str, folder: Option<&str>) -> String {
        let placemark = store
            .create_placemark(point(), folder.map(str::to_owned))
            .expect("placemark");
        store
            .update_placemark(
                &placemark.id,
                PlacemarkPatch {
                    name: Patch::Set(name.into()),
                    ..Default::default()
                },
            )
            .expect("rename");
        placemark.id
    }

    /// root: A(open){ B(open){ b1 }, a1 }, C, c1 (root placemark)
    struct Fixture {
        store: StateStore,
        a: String,
        b: String,
        c: String,
        a1: String,
        b1: String,
        c1: String,
    }

    fn fixture() -> Fixture {
        let mut store = store();
        let a = store.create_folder("Alpha", None).expect("a").id;
        let b = store.create_folder("Beta", Some(a.clone())).expect("b").id;
        let c = store.create_folder("Gamma", None).expect("c").id;
        let b1 = add_placemark(&mut store, "bravo", Some(&b));
        let a1 = add_placemark(&mut store, "apple", Some(&a));
        let c1 = add_placemark(&mut store, "cherry", None);
        Fixture {
            store,
            a,
            b,
            c,
            a1,
            b1,
            c1,
        }
    }

    fn ids(rows: &[VisibleRow]) -> Vec<&str> {
        rows.iter().map(|row| row.id.as_str()).collect()
    }

    #[test]
    fn empty_library_has_no_rows() {
        let store = store();
        assert!(visible_rows(store.library(), "").is_empty());
    }

    #[test]
    fn nested_rows_come_out_depth_first_folders_before_placemarks() {
        let f = fixture();
        let rows = visible_rows(f.store.library(), "");
        assert_eq!(
            ids(&rows),
            [&f.a, &f.b, &f.b1, &f.a1, &f.c, &f.c1].map(String::as_str)
        );
        let depths: Vec<usize> = rows.iter().map(|row| row.depth).collect();
        assert_eq!(depths, [0, 1, 2, 1, 0, 0]);
    }

    #[test]
    fn closed_folder_hides_its_children_but_reports_them() {
        let mut f = fixture();
        f.store.set_folder_open(&f.a, false).expect("close");
        let rows = visible_rows(f.store.library(), "");
        assert_eq!(ids(&rows), [&f.a, &f.c, &f.c1].map(String::as_str));
        let a = &rows[0];
        assert!(a.has_children && !a.open);
    }

    #[test]
    fn empty_folder_has_no_children() {
        let f = fixture();
        let rows = visible_rows(f.store.library(), "");
        let c = rows.iter().find(|row| row.id == f.c).expect("c");
        assert!(!c.has_children);
    }

    #[test]
    fn sibling_flags_track_what_follows_each_row() {
        let f = fixture();
        let rows = visible_rows(f.store.library(), "");
        let flags = |id: &str| {
            let row = rows.iter().find(|row| row.id == id).expect("row");
            (
                row.has_sibling_below,
                row.ancestors_have_sibling_below.clone(),
            )
        };
        assert_eq!(flags(&f.a), (true, vec![]));
        assert_eq!(flags(&f.b), (true, vec![true]));
        assert_eq!(flags(&f.b1), (false, vec![true, true]));
        assert_eq!(flags(&f.a1), (false, vec![true]));
        assert_eq!(flags(&f.c), (true, vec![]));
        assert_eq!(flags(&f.c1), (false, vec![]));
    }

    #[test]
    fn search_keeps_ancestors_of_matches_and_drops_the_rest() {
        let f = fixture();
        let rows = visible_rows(f.store.library(), "bravo");
        assert_eq!(ids(&rows), [&f.a, &f.b, &f.b1].map(String::as_str));
        let a = &rows[0];
        assert!(!a.has_sibling_below, "unmatched siblings do not count");
    }

    #[test]
    fn search_without_matches_is_empty() {
        let f = fixture();
        assert!(visible_rows(f.store.library(), "zzz").is_empty());
    }

    #[test]
    fn geometry_picks_the_icon() {
        assert_eq!(icon_for_geometry(&point()), IconKind::Point);
        let circle = PlacemarkGeometry::Circle {
            center: GeoPoint::new(0.0, 0.0).expect("point"),
            radius_meters: 5.0,
        };
        assert_eq!(icon_for_geometry(&circle), IconKind::Circle);
    }

    fn nav_rows() -> (Fixture, Vec<VisibleRow>) {
        let f = fixture();
        let rows = visible_rows(f.store.library(), "");
        (f, rows)
    }

    fn select(id: &str) -> NavAction {
        NavAction::Select(id.to_owned())
    }

    #[test]
    fn down_and_up_move_over_visible_rows() {
        let (f, rows) = nav_rows();
        assert_eq!(
            next_selection(&rows, Some(&f.a), NavKey::Down),
            select(&f.b)
        );
        assert_eq!(next_selection(&rows, Some(&f.b), NavKey::Up), select(&f.a));
    }

    #[test]
    fn up_on_first_and_down_on_last_stay_put() {
        let (f, rows) = nav_rows();
        assert_eq!(
            next_selection(&rows, Some(&f.a), NavKey::Up),
            NavAction::None
        );
        assert_eq!(
            next_selection(&rows, Some(&f.c1), NavKey::Down),
            NavAction::None
        );
    }

    #[test]
    fn nothing_selected_selects_first_on_down_only() {
        let (f, rows) = nav_rows();
        assert_eq!(next_selection(&rows, None, NavKey::Down), select(&f.a));
        for key in [NavKey::Up, NavKey::Left, NavKey::Right, NavKey::Enter] {
            assert_eq!(next_selection(&rows, None, key), NavAction::None);
        }
        assert_eq!(next_selection(&[], None, NavKey::Down), NavAction::None);
        assert_eq!(
            next_selection(&rows, Some("gone"), NavKey::Down),
            select(&f.a)
        );
    }

    #[test]
    fn left_collapses_an_open_folder_then_goes_to_the_parent() {
        let (f, rows) = nav_rows();
        assert_eq!(
            next_selection(&rows, Some(&f.a), NavKey::Left),
            NavAction::Collapse(f.a.clone())
        );
        assert_eq!(
            next_selection(&rows, Some(&f.b1), NavKey::Left),
            select(&f.b)
        );
        assert_eq!(
            next_selection(&rows, Some(&f.a1), NavKey::Left),
            select(&f.a)
        );
    }

    #[test]
    fn left_on_a_root_row_without_open_children_does_nothing() {
        let (f, rows) = nav_rows();
        assert_eq!(
            next_selection(&rows, Some(&f.c), NavKey::Left),
            NavAction::None
        );
        assert_eq!(
            next_selection(&rows, Some(&f.c1), NavKey::Left),
            NavAction::None
        );

        let mut f = fixture();
        f.store.set_folder_open(&f.a, false).expect("close");
        let rows = visible_rows(f.store.library(), "");
        assert_eq!(
            next_selection(&rows, Some(&f.a), NavKey::Left),
            NavAction::None
        );
    }

    #[test]
    fn right_expands_a_closed_folder_then_enters_it() {
        let mut f = fixture();
        f.store.set_folder_open(&f.a, false).expect("close");
        let rows = visible_rows(f.store.library(), "");
        assert_eq!(
            next_selection(&rows, Some(&f.a), NavKey::Right),
            NavAction::Expand(f.a.clone())
        );
        let (f, rows) = nav_rows();
        assert_eq!(
            next_selection(&rows, Some(&f.a), NavKey::Right),
            select(&f.b)
        );
    }

    #[test]
    fn right_on_a_placemark_or_childless_folder_does_nothing() {
        let (f, rows) = nav_rows();
        assert_eq!(
            next_selection(&rows, Some(&f.c1), NavKey::Right),
            NavAction::None
        );
        assert_eq!(
            next_selection(&rows, Some(&f.c), NavKey::Right),
            NavAction::None
        );
    }

    #[test]
    fn enter_flies_to_the_current_row() {
        let (f, rows) = nav_rows();
        assert_eq!(
            next_selection(&rows, Some(&f.b1), NavKey::Enter),
            NavAction::FlyTo(f.b1.clone())
        );
    }
}
