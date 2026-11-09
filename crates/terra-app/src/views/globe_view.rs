//! Central globe viewport: input, keyboard navigation, overlays, and the HUD.

use eframe::egui;
use terra_core::{GeoPoint, PlacemarkGeometry, format_coordinate, format_distance};

use crate::i18n::I18n;
use crate::tools::RouteMode;
use crate::{AppData, theme};
use web_time::Instant;

use super::nav_controls;
use super::top_bar::create_placemark;
use super::tr;

pub(super) fn central_globe(
    data: &mut AppData,
    i18n: &I18n,
    ui: &mut egui::Ui,
    render_state: Option<&egui_wgpu::RenderState>,
) -> egui::Rect {
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(theme::SPACE))
        .show(ui, |ui| {
            let available = ui.available_rect_before_wrap();
            let pixels_per_point = ui.ctx().pixels_per_point();
            let width = (available.width() * pixels_per_point).round().max(1.0) as u32;
            let height = (available.height() * pixels_per_point).round().max(1.0) as u32;
            let scale = f64::from(pixels_per_point);
            let (rect, response) =
                ui.allocate_exact_size(available.size(), egui::Sense::click_and_drag());

            data.globe.resize(width, height);
            let nav = nav_controls::interact(data, i18n, ui, rect);
            handle_globe_input(data, ui, rect, &response);
            data.cursor_world = ui
                .input(|input| input.pointer.hover_pos())
                .filter(|_| response.hovered())
                .and_then(|pos| {
                    data.globe.pick(pixel_pos(pos, rect, scale)).map(|hit| {
                        GeoPoint::new(hit.lon_deg, hit.lat_deg).unwrap_or(GeoPoint {
                            lon: 0.0,
                            lat: 0.0,
                            altitude: None,
                            altitude_mode: None,
                        })
                    })
                });

            let (revision, scene) = data.build_scene();
            data.globe.set_scene(revision, scene);

            let start = Instant::now();
            data.globe.render(data.now_seconds());
            register_texture(data, render_state);
            if let Some(texture) = data.texture {
                ui.painter().image(
                    texture,
                    rect,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
            }
            draw_overlays(data, i18n, ui, rect, scale);
            draw_hud(data, ui, rect);
            nav_controls::paint(ui, rect, data, &nav);
            let elapsed = start.elapsed();
            data.frame_ms = data.frame_ms * 0.9 + elapsed.as_secs_f32() * 1000.0 * 0.1;

            // Keep animating while tiles or camera animations are in flight: the
            // fetcher is only pumped from `render`, so a slow idle repaint would
            // throttle imagery streaming to a couple of tiles per second.
            let busy = data.globe.camera.is_animating()
                || data.globe.is_loading_imagery()
                || data.route.pending
                || data.geocode.pending;
            ui.ctx().request_repaint_after(if busy {
                std::time::Duration::from_millis(16)
            } else {
                std::time::Duration::from_millis(120)
            });
            rect
        })
        .inner
}

fn pixel_pos(pos: egui::Pos2, rect: egui::Rect, pixels_per_point: f64) -> (f64, f64) {
    (
        f64::from(pos.x - rect.min.x) * pixels_per_point,
        f64::from(pos.y - rect.min.y) * pixels_per_point,
    )
}

fn handle_globe_input(
    data: &mut AppData,
    ui: &egui::Ui,
    rect: egui::Rect,
    response: &egui::Response,
) {
    let pixels_per_point = f64::from(ui.ctx().pixels_per_point());
    let cursor_pixel = ui
        .input(|input| input.pointer.hover_pos())
        .map(|pos| pixel_pos(pos, rect, pixels_per_point))
        .unwrap_or((
            f64::from(rect.width()) * pixels_per_point * 0.5,
            f64::from(rect.height()) * pixels_per_point * 0.5,
        ));
    let (shift, secondary_down, middle_down, pointer_delta) = ui.input(|input| {
        (
            input.modifiers.shift,
            input.pointer.button_down(egui::PointerButton::Secondary),
            input.pointer.button_down(egui::PointerButton::Middle),
            input.pointer.delta(),
        )
    });

    // Left drag moves the globe, the way Google Earth does. Shift turns the
    // drag into tilt/rotate instead.
    if response.dragged_by(egui::PointerButton::Primary) && !data.drawing.is_active() {
        let delta = response.drag_delta();
        if shift {
            data.globe.orbit(f64::from(delta.x), f64::from(delta.y));
        } else {
            // The globe works in physical pixels; egui reports drags in points.
            data.globe.pan_pixels(
                cursor_pixel,
                (
                    f64::from(delta.x) * pixels_per_point,
                    f64::from(delta.y) * pixels_per_point,
                ),
            );
        }
    }

    if response.hovered() {
        // Right drag: up/down zooms towards the cursor, left/right rotates.
        if secondary_down {
            if pointer_delta.y.abs() > 0.0 {
                data.globe
                    .zoom_at(cursor_pixel, (f64::from(pointer_delta.y) * 0.01).exp());
            }
            if pointer_delta.x.abs() > 0.0 {
                data.globe.rotate_by(f64::from(pointer_delta.x) * 0.005);
            }
        }
        // Middle drag tilts and rotates, like Google Earth's navigation pad.
        if middle_down {
            data.globe
                .orbit(f64::from(pointer_delta.x), f64::from(pointer_delta.y));
        }

        let (scroll, pinch) = ui.input(|input| (input.smooth_scroll_delta.y, input.zoom_delta()));
        if (pinch - 1.0).abs() > 0.01 {
            data.globe.zoom_at(cursor_pixel, 1.0 / f64::from(pinch));
        } else if scroll.abs() > 0.5 {
            data.globe
                .zoom_at(cursor_pixel, (-f64::from(scroll) * 0.003).exp());
        }

        if let Some(pos) = ui.input(|input| input.pointer.hover_pos()) {
            data.hovered = data
                .globe
                .pick_placemark(pixel_pos(pos, rect, pixels_per_point))
                .map(|hit| hit.id);
        }
    } else {
        data.hovered = None;
    }

    globe_keyboard(data, ui, response);

    if response.clicked() {
        response.request_focus();
        let Some(pos) = response.interact_pointer_pos() else {
            return;
        };
        let pixel = pixel_pos(pos, rect, pixels_per_point);
        let surface = data
            .globe
            .pick(pixel)
            .and_then(|hit| GeoPoint::new(hit.lon_deg, hit.lat_deg).ok());

        if data.route_adding {
            if let Some(point) = surface {
                data.route.waypoints.push(point);
            }
            return;
        }
        if data.ruler_adding {
            if let Some(point) = surface {
                data.ruler.add(point);
            }
            return;
        }
        if data.drawing.is_active() {
            let Some(point) = surface else {
                return;
            };
            if let Some(geometry) = data.drawing.click(point) {
                create_placemark(data, geometry);
            }
            return;
        }

        match data.globe.pick_placemark(pixel) {
            Some(hit) => data.select(Some(hit.id)),
            None => {
                data.close_editor();
                data.selected_folder = None;
            }
        }
    }

    // Google Earth: double-click zooms towards the cursor, right double-click
    // zooms away from it.
    let adding = data.drawing.is_active() || data.ruler_adding || data.route_adding;
    if response.double_clicked_by(egui::PointerButton::Secondary) {
        data.globe.zoom_at(cursor_pixel, 2.5);
    } else if response.double_clicked() && !adding {
        let Some(pos) = response.interact_pointer_pos() else {
            return;
        };
        let pixel = pixel_pos(pos, rect, pixels_per_point);
        match data.globe.pick_placemark(pixel).map(|hit| hit.id) {
            Some(id) => {
                let now = data.now_seconds();
                if let Some(geometry) = data
                    .store
                    .library()
                    .placemark(&id)
                    .map(|placemark| placemark.geometry.clone())
                {
                    data.globe.fly_to_geometry(&geometry, now);
                }
            }
            None => data.globe.zoom_at(pixel, 0.5),
        }
    }
}

/// Rotation speed for shift+arrow, radians per second.
const ROTATE_RATE: f64 = 0.9;

/// Tilt speed for shift+arrow, radians per second.
const TILT_RATE: f64 = 0.6;

/// Pan speed as a fraction of the camera distance per second.
pub(super) const PAN_RATE: f64 = 0.4;

/// Camera distance factor for one press of `+`/Page Up; `-`/Page Down use its inverse.
pub(super) const ZOOM_STEP: f64 = 0.75;

/// Google Earth's 3D-viewer keys: arrows move, shift+arrows rotate and tilt,
/// `+`/`-`/Page Up/Page Down zoom, `n`/`u`/`r` reset the orientation and space
/// stops a fly-to.
fn globe_keyboard(data: &mut AppData, ui: &egui::Ui, response: &egui::Response) {
    use egui::Key;
    let editing_text = ui
        .memory(|memory| memory.focused())
        .is_some_and(|id| id != response.id);
    if editing_text || !(response.has_focus() || response.hovered()) {
        data.globe_key_hold = 0.0;
        return;
    }

    let (shift, alt, held, pressed, tapped, dt) = ui.input(|input| {
        let down = |key: Key| input.key_down(key);
        (
            input.modifiers.shift,
            input.modifiers.alt,
            [
                down(Key::ArrowLeft),
                down(Key::ArrowRight),
                down(Key::ArrowUp),
                down(Key::ArrowDown),
            ],
            (
                input.key_pressed(Key::Plus) || input.key_pressed(Key::Equals),
                input.key_pressed(Key::Minus),
                input.key_pressed(Key::PageUp),
                input.key_pressed(Key::PageDown),
                input.key_pressed(Key::N),
                input.key_pressed(Key::U),
                input.key_pressed(Key::R),
                input.key_pressed(Key::Space),
            ),
            input.key_pressed(Key::ArrowLeft)
                || input.key_pressed(Key::ArrowRight)
                || input.key_pressed(Key::ArrowUp)
                || input.key_pressed(Key::ArrowDown),
            f64::from(input.stable_dt).clamp(0.001, 0.1),
        )
    });
    let [left, right, up, down] = held;
    let (zoom_in, zoom_out, page_up, page_down, north, top_down, reset, stop) = pressed;

    if north {
        data.globe.set_north_up();
    }
    if top_down {
        data.globe.set_top_down();
    }
    if reset {
        data.globe.reset_view();
    }
    if stop {
        data.globe.stop_motion();
    }
    if zoom_in || page_up {
        data.globe.zoom_by(ZOOM_STEP);
    }
    if zoom_out || page_down {
        data.globe.zoom_by(1.0 / ZOOM_STEP);
    }

    // A single tap still has to turn or move a visible amount, and Google
    // Earth's Alt modifier makes every keyboard move slower.
    let tap_dt = if tapped { dt.max(1.0 / 12.0) } else { dt };
    let slow = if alt { 0.25 } else { 1.0 };
    let mut busy = false;
    if shift {
        // Shift+left/right rotate the imagery, shift+up tilts towards top-down
        // and shift+down towards the horizon.
        if left || right {
            let direction = f64::from(i32::from(right) - i32::from(left));
            data.globe
                .rotate_by(direction * ROTATE_RATE * tap_dt * slow);
            busy = true;
        }
        if up || down {
            let direction = f64::from(i32::from(up) - i32::from(down));
            data.globe.tilt_by(direction * TILT_RATE * tap_dt * slow);
            busy = true;
        }
    } else if left || right || up || down {
        // Google Earth moves a fraction of the current altitude per second and
        // speeds up while a key is held.
        data.globe_key_hold = (data.globe_key_hold + dt as f32).min(1.5);
        let step_dt = tap_dt;
        let speed = PAN_RATE * f64::from(1.0 + 2.0 * data.globe_key_hold);
        let step = data.globe.view_distance() * speed * step_dt * slow;
        let forward = f64::from(i32::from(up) - i32::from(down)) * step;
        let sideways = f64::from(i32::from(right) - i32::from(left)) * step;
        data.globe.move_view(forward, sideways);
        busy = true;
    } else {
        data.globe_key_hold = 0.0;
    }

    if busy || zoom_in || zoom_out || page_up || page_down || reset {
        ui.ctx().request_repaint();
    }
}

fn register_texture(data: &mut AppData, render_state: Option<&egui_wgpu::RenderState>) {
    let generation = data.globe.renderer.generation();
    if data.texture.is_some() && generation == data.registered_generation {
        return;
    }
    let Some(render_state) = render_state else {
        return;
    };
    if let Some(texture) = data.texture.take() {
        render_state.renderer.write().free_texture(&texture);
    }
    let texture = render_state.renderer.write().register_native_texture(
        &render_state.device,
        data.globe.renderer.texture_view(),
        egui_wgpu::wgpu::FilterMode::Linear,
    );
    data.texture = Some(texture);
    data.registered_generation = generation;
}

fn draw_overlays(
    data: &AppData,
    i18n: &I18n,
    ui: &egui::Ui,
    rect: egui::Rect,
    pixels_per_point: f64,
) {
    let painter = ui.painter().with_clip_rect(rect);
    let viewport = data.globe.viewport();

    let mut marker_count = 0usize;
    let mut labels: Vec<(egui::Rect, u8, std::sync::Arc<egui::Galley>)> = Vec::new();
    let mut pins: Vec<egui::Rect> = Vec::new();
    for placemark in data.store.library().visible_placemarks() {
        if marker_count > 600 {
            break;
        }
        let style = data.store.library().style_for(placemark);
        let selected = data.selected.as_deref() == Some(placemark.id.as_str());
        let hovered = data.hovered.as_deref() == Some(placemark.id.as_str());

        match &placemark.geometry {
            PlacemarkGeometry::Point { coordinates }
            | PlacemarkGeometry::Model {
                position: coordinates,
                ..
            } => {
                let screen = marker_screen_position(
                    &data.globe.camera,
                    coordinates,
                    viewport,
                    &|lon, lat| data.globe.ground_height(lon, lat),
                );
                let Some(screen) = screen else { continue };
                let position = egui::pos2(
                    rect.min.x + (screen.x / pixels_per_point) as f32,
                    rect.min.y + (screen.y / pixels_per_point) as f32,
                );
                if !rect.contains(position) {
                    continue;
                }
                let fill = theme::color32_from_hex(&style.fill_color);
                let outline = if selected {
                    theme::ACCENT
                } else if hovered {
                    egui::Color32::WHITE
                } else {
                    theme::color32_from_hex(&style.outline_color)
                };
                draw_marker(
                    &painter,
                    position,
                    style.marker_icon,
                    fill,
                    outline,
                    selected,
                );
                marker_count += 1;
                pins.push(egui::Rect::from_center_size(
                    position,
                    egui::Vec2::splat(2.0 * marker_radius(selected)),
                ));
                if data.settings.show_labels && data.globe.view_distance() < 3_000_000.0 {
                    let priority = label_priority(selected, hovered, 2);
                    let galley = painter.layout_no_wrap(
                        placemark.name.clone(),
                        egui::FontId::proportional(label_size(data.globe.view_distance())),
                        theme::TEXT,
                    );
                    let at = position + egui::vec2(10.0 * MARKER_SCALE, 4.0 * MARKER_SCALE);
                    let label_rect = egui::Align2::LEFT_TOP.anchor_size(at, galley.size());
                    labels.push((label_rect, priority, galley));
                }
            }
            _ => {
                if !data.settings.show_labels {
                    continue;
                }
                let (center, align) = label_anchor(&placemark.geometry);
                let ground = data
                    .globe
                    .ground_height(center.lon, center.lat)
                    .unwrap_or(0.0);
                let Some(screen) = data.globe.project(terra_globe::Geodetic::from_degrees(
                    center.lon, center.lat, ground,
                )) else {
                    continue;
                };
                let position = egui::pos2(
                    rect.min.x + (screen.x / pixels_per_point) as f32,
                    rect.min.y + (screen.y / pixels_per_point) as f32,
                );
                if !rect.contains(position) || data.globe.view_distance() > 2_000_000.0 {
                    continue;
                }
                let galley = painter.layout_no_wrap(
                    placemark.name.clone(),
                    egui::FontId::proportional(label_size(data.globe.view_distance())),
                    if selected { theme::ACCENT } else { theme::TEXT },
                );
                let label_rect = align.anchor_size(position, galley.size());
                labels.push((label_rect, label_priority(selected, hovered, 3), galley));
            }
        }
    }

    let rects: Vec<_> = labels.iter().map(|(r, p, _)| (*r, *p)).collect();
    for index in declutter(&rects, &pins) {
        if let Some((label_rect, _, galley)) = labels.get(index) {
            paint_label(&painter, label_rect.min, galley);
        }
    }

    for overlay in &data.store.library().overlays {
        if !overlay.visibility {
            continue;
        }
        let position = anchor_position(
            &overlay.screen_xy,
            rect,
            overlay.size_x.unwrap_or(140.0) as f32,
            overlay.size_y.unwrap_or(40.0) as f32,
        );
        let size = egui::vec2(
            overlay.size_x.unwrap_or(140.0) as f32,
            overlay.size_y.unwrap_or(40.0) as f32,
        );
        let overlay_rect = egui::Rect::from_min_size(position, size);
        painter.rect_filled(
            overlay_rect,
            3.0,
            theme::SURFACE_RAISED.gamma_multiply(0.85),
        );
        painter.rect_stroke(
            overlay_rect,
            3.0,
            egui::Stroke::new(1.0, theme::ACCENT),
            egui::StrokeKind::Inside,
        );
        painter.text(
            overlay_rect.center(),
            egui::Align2::CENTER_CENTER,
            &overlay.name,
            egui::FontId::proportional(11.0),
            theme::TEXT,
        );
    }

    if data.ruler_adding || data.ruler.points.len() >= 2 {
        if let Some(last) = data.ruler.points.last() {
            if data.settings.show_labels {
                let _ = format_coordinate(last, data.settings.coordinate_format);
            }
        }
    }
    let _ = i18n;
}

/// Resolves a KML ScreenOverlay anchor into a viewport position.
fn anchor_position(
    anchor: &terra_core::ScreenOverlayAnchor,
    rect: egui::Rect,
    width: f32,
    height: f32,
) -> egui::Pos2 {
    use terra_core::AnchorUnits;
    let resolve = |value: f64, units: AnchorUnits, span: f32, size: f32| -> f32 {
        match units {
            AnchorUnits::Fraction => value as f32 * span,
            AnchorUnits::Pixels => value as f32,
            AnchorUnits::InsetPixels => span - size - value as f32,
        }
    };
    egui::pos2(
        rect.min.x + resolve(anchor.x, anchor.x_units, rect.width(), width),
        rect.min.y + resolve(anchor.y, anchor.y_units, rect.height(), height),
    )
}

/// Paths are labelled just above their northernmost point: concentric rings (range rings
/// around a stand) would otherwise stack every label on the shared centre.
fn label_anchor(geometry: &PlacemarkGeometry) -> (GeoPoint, egui::Align2) {
    if let PlacemarkGeometry::LineString { path, .. } = geometry {
        if let Some(top) = path.iter().max_by(|a, b| a.lat.total_cmp(&b.lat)) {
            return (*top, egui::Align2::CENTER_BOTTOM);
        }
    }
    (
        terra_core::geometry_center(geometry),
        egui::Align2::CENTER_CENTER,
    )
}

fn label_priority(selected: bool, hovered: bool, base: u8) -> u8 {
    if selected {
        0
    } else if hovered {
        1
    } else {
        base
    }
}

/// Light text with a dark halo, so labels stay legible over bright imagery.
fn paint_label(painter: &egui::Painter, at: egui::Pos2, galley: &std::sync::Arc<egui::Galley>) {
    let halo = egui::Color32::from_black_alpha(170);
    for offset in [
        egui::vec2(-1.0, 0.0),
        egui::vec2(1.0, 0.0),
        egui::vec2(0.0, -1.0),
        egui::vec2(0.0, 1.0),
    ] {
        painter.galley_with_override_text_color(at + offset, galley.clone(), halo);
    }
    painter.galley(at, galley.clone(), theme::TEXT);
}

/// Labels of this priority or better (selected, hovered) may cover marker pins.
const PIN_EXEMPT_PRIORITY: u8 = 1;

/// Greedy label decluttering: indices of the labels to draw, best priority first
/// (lower number wins, ties keep input order). A label is dropped when its rect,
/// inflated by 2 px, overlaps one already accepted, or when it covers a marker pin
/// and is not the selected or hovered label. Rects separated by more than the margin both stay.
fn declutter(candidates: &[(egui::Rect, u8)], pins: &[egui::Rect]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..candidates.len()).collect();
    order.sort_by_key(|&i| candidates.get(i).map_or(u8::MAX, |c| c.1));
    let mut accepted: Vec<egui::Rect> = Vec::new();
    let mut kept = Vec::new();
    for i in order {
        let Some(&(rect, _)) = candidates.get(i) else {
            continue;
        };
        let padded = rect.expand(1.0);
        let covers_pin = candidates.get(i).is_some_and(|c| c.1 > PIN_EXEMPT_PRIORITY)
            && pins.iter().any(|pin| padded.intersects(*pin));
        if !covers_pin && accepted.iter().all(|a| !padded.intersects(a.expand(1.0))) {
            accepted.push(rect);
            kept.push(i);
        }
    }
    kept
}

fn label_size(distance: f64) -> f32 {
    let scaled = 18.0 - (distance / 2_000_000.0).clamp(0.0, 1.0) * 7.0;
    scaled.clamp(11.0, 18.0) as f32
}

/// Scales every placemark marker: radius, icon offsets, stroke widths and the
/// label offset, so each icon keeps its proportions. Owned by terra-globe,
/// whose picking derives the hit radius from the same value.
pub(crate) const MARKER_SCALE: f32 = terra_globe::picking::MARKER_SCALE;

/// Marker radius in points, before icon-specific proportions are applied.
fn marker_radius(selected: bool) -> f32 {
    (if selected { 8.0 } else { 6.0 }) * MARKER_SCALE
}

fn draw_marker(
    painter: &egui::Painter,
    position: egui::Pos2,
    icon: Option<terra_core::MarkerIconId>,
    fill: egui::Color32,
    outline: egui::Color32,
    selected: bool,
) {
    let k = MARKER_SCALE;
    let radius = marker_radius(selected);
    let icon = icon.unwrap_or(terra_core::DEFAULT_MARKER_ICON);
    let stroke = egui::Stroke::new(if selected { 2.0 } else { 1.5 } * k, outline);
    match icon {
        terra_core::MarkerIconId::Star => {
            let mut points = Vec::new();
            for index in 0..10 {
                let angle =
                    -std::f32::consts::FRAC_PI_2 + index as f32 * std::f32::consts::PI / 5.0;
                let length = if index % 2 == 0 {
                    radius + 2.0 * k
                } else {
                    radius * 0.45
                };
                points.push(position + egui::vec2(angle.cos(), angle.sin()) * length);
            }
            painter.add(egui::Shape::convex_polygon(points, fill, stroke));
        }
        terra_core::MarkerIconId::Flag => {
            painter.line_segment(
                [
                    position - egui::vec2(0.0, radius),
                    position + egui::vec2(0.0, radius * 1.4),
                ],
                egui::Stroke::new(1.8 * k, outline),
            );
            painter.add(egui::Shape::convex_polygon(
                vec![
                    position - egui::vec2(0.0, radius),
                    position + egui::vec2(radius + 3.0 * k, -radius + 3.0 * k),
                    position + egui::vec2(0.0, -radius + 7.0 * k),
                ],
                fill,
                stroke,
            ));
        }
        terra_core::MarkerIconId::Navigation => {
            painter.add(egui::Shape::convex_polygon(
                vec![
                    position + egui::vec2(0.0, -radius - 2.0 * k),
                    position + egui::vec2(radius + k, radius),
                    position + egui::vec2(-radius - k, radius),
                ],
                fill,
                stroke,
            ));
        }
        terra_core::MarkerIconId::Landmark => {
            painter.add(egui::Shape::convex_polygon(
                vec![
                    position + egui::vec2(-radius, -radius * 0.2),
                    position + egui::vec2(0.0, -radius - 2.0 * k),
                    position + egui::vec2(radius, -radius * 0.2),
                ],
                outline,
                stroke,
            ));
            painter.rect_filled(
                egui::Rect::from_center_size(
                    position + egui::vec2(0.0, radius * 0.4),
                    egui::vec2(radius * 1.6, radius),
                ),
                2.0,
                fill,
            );
        }
        terra_core::MarkerIconId::Pin => {
            painter.circle_filled(position - egui::vec2(0.0, radius * 0.4), radius * 0.7, fill);
            painter.line_segment(
                [
                    position - egui::vec2(0.0, radius * 0.4),
                    position + egui::vec2(0.0, radius),
                ],
                egui::Stroke::new(1.8 * k, outline),
            );
        }
        terra_core::MarkerIconId::MapPin => {
            painter.circle(position, radius, fill, stroke);
            painter.add(egui::Shape::convex_polygon(
                vec![
                    position + egui::vec2(-radius * 0.75, radius * 0.6),
                    position + egui::vec2(0.0, radius * 1.9),
                    position + egui::vec2(radius * 0.75, radius * 0.6),
                ],
                fill,
                stroke,
            ));
        }
    }
}

/// Each credit may itself be a " | "-joined list; repeated parts are shown once.
fn merge_credits(credits: &[String]) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for part in credits.iter().flat_map(|credit| credit.split('|')) {
        let part = part.trim();
        if !part.is_empty() && !parts.contains(&part) {
            parts.push(part);
        }
    }
    parts.join("  |  ")
}

/// Height of the translucent status strip that carries credits and the cursor position.
const STATUS_HEIGHT: f32 = 20.0;
const STATUS_PAD: f32 = 8.0;

fn draw_hud(data: &AppData, ui: &egui::Ui, rect: egui::Rect) {
    let painter = ui.painter().with_clip_rect(rect);
    let strip = egui::Rect::from_min_max(
        egui::pos2(rect.left(), rect.bottom() - STATUS_HEIGHT),
        rect.right_bottom(),
    );
    // Credits must stay legible over bright satellite imagery.
    painter.rect_filled(strip, 0.0, egui::Color32::from_black_alpha(150));
    let font = egui::FontId::proportional(11.0);
    let mut credits_right = strip.right() - STATUS_PAD;
    if let Some(point) = data.cursor_world {
        let galley = painter.layout_no_wrap(
            format_coordinate(&point, data.settings.coordinate_format),
            font.clone(),
            theme::TEXT,
        );
        let position = egui::pos2(
            strip.right() - STATUS_PAD - galley.size().x,
            strip.center().y - galley.size().y / 2.0,
        );
        credits_right = position.x - 2.0 * STATUS_PAD;
        painter.galley(position, galley, theme::TEXT);
    }
    let mut job = egui::text::LayoutJob::simple_singleline(
        merge_credits(&data.globe.credits()),
        font,
        theme::TEXT,
    );
    job.wrap = egui::text::TextWrapping::truncate_at_width(
        (credits_right - strip.left() - STATUS_PAD).max(0.0),
    );
    let galley = painter.layout_job(job);
    painter.galley(
        egui::pos2(
            strip.left() + STATUS_PAD,
            strip.center().y - galley.size().y / 2.0,
        ),
        galley,
        theme::TEXT,
    );
    if let Some(error) = &data.error_banner {
        painter.text(
            rect.center_top() + egui::vec2(0.0, 12.0),
            egui::Align2::CENTER_TOP,
            error,
            egui::FontId::proportional(13.0),
            theme::DANGER,
        );
    }
    if data.show_stats {
        let stats = data.globe.renderer.stats();
        let lines = format!(
            "{:.1} ms/frame\n{} tiles drawn\n{} textures\n{} requests\nlevel {}\n{} scene items",
            data.frame_ms,
            stats.tiles_drawn,
            stats.textures_ready,
            stats.requests_in_flight,
            stats.deepest_level,
            stats.scene_items
        );
        painter.text(
            rect.right_top() + egui::vec2(-10.0, 10.0),
            egui::Align2::RIGHT_TOP,
            lines,
            egui::FontId::monospace(11.0),
            theme::TEXT_MUTED,
        );
    }
}

pub(super) fn floating_panels(data: &mut AppData, i18n: &I18n, ctx: &egui::Context) {
    if !data.ruler.points.is_empty() {
        let mut open = true;
        egui::Window::new(tr(i18n, "ruler.panelTitle"))
            .open(&mut open)
            .default_pos([320.0, 560.0])
            .resizable(false)
            .show(ctx, |ui| {
                let segments = data.ruler.segment_meters();
                for (index, distance) in segments.iter().enumerate() {
                    ui.label(format!(
                        "{} {}: {}",
                        tr(i18n, "ruler.segment"),
                        index + 1,
                        format_distance(*distance, data.settings.unit_system)
                    ));
                }
                ui.separator();
                ui.label(format!(
                    "{}: {}",
                    tr(i18n, "ruler.total"),
                    format_distance(data.ruler.total_meters(), data.settings.unit_system)
                ));
                ui.horizontal(|ui| {
                    if ui.button(tr(i18n, "ruler.undo")).clicked() {
                        data.ruler.undo();
                    }
                    if ui.button(tr(i18n, "ruler.finish")).clicked() {
                        data.ruler.finish();
                        data.ruler_adding = false;
                    }
                    if ui.button(tr(i18n, "ruler.close")).clicked() {
                        data.ruler.clear();
                        data.ruler_adding = false;
                    }
                });
            });
        if !open {
            data.ruler.clear();
        }
    }

    if data.route.is_active() || data.route.pending {
        let mut open = true;
        egui::Window::new(tr(i18n, "routePlanner.panelTitle"))
            .open(&mut open)
            .default_pos([360.0, 200.0])
            .default_width(300.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(tr(i18n, "routePlanner.mode"));
                    let mode = data.route.mode.unwrap_or(RouteMode::Car);
                    egui::ComboBox::from_id_salt("route-mode")
                        .selected_text(tr(i18n, mode.label_key()))
                        .show_ui(ui, |ui| {
                            for candidate in [
                                RouteMode::Car,
                                RouteMode::Foot,
                                RouteMode::Bike,
                                RouteMode::Plane,
                                RouteMode::Train,
                            ] {
                                if ui
                                    .selectable_label(
                                        mode == candidate,
                                        tr(i18n, candidate.label_key()),
                                    )
                                    .clicked()
                                {
                                    data.route.mode = Some(candidate);
                                }
                            }
                        });
                });
                ui.label(format!(
                    "{} {}",
                    tr(i18n, "routePlanner.stop"),
                    data.route.waypoints.len()
                ));
                let mut remove = None;
                for (index, waypoint) in data.route.waypoints.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(format!(
                            "{}. {}",
                            index + 1,
                            format_coordinate(waypoint, data.settings.coordinate_format)
                        ));
                        if ui
                            .small_button("x")
                            .on_hover_text(tr(i18n, "routePlanner.removeStop"))
                            .clicked()
                        {
                            remove = Some(index);
                        }
                    });
                }
                if let Some(index) = remove {
                    data.route.waypoints.remove(index);
                }
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(
                            !data.route.pending,
                            egui::Button::new(tr(i18n, "routePlanner.calculate")),
                        )
                        .clicked()
                    {
                        data.start_route();
                    }
                    if ui.button(tr(i18n, "routePlanner.clear")).clicked() {
                        data.route.clear();
                        data.route_adding = false;
                    }
                    if data.route.pending {
                        ui.spinner();
                        ui.label(tr(i18n, "routePlanner.calculating"));
                    }
                });
                if let Some(error) = &data.route.error {
                    ui.colored_label(theme::DANGER, error);
                }
                for (index, leg) in data.route.legs.iter().enumerate() {
                    ui.label(format!(
                        "{} {} - {}",
                        tr(i18n, "routePlanner.routeAlternative")
                            .replace("{{n}}", &(index + 1).to_string()),
                        format_distance(leg.distance_meters, data.settings.unit_system),
                        terra_core::format_duration(leg.duration_seconds)
                    ));
                }
                if !data.route.legs.is_empty() {
                    ui.label(
                        egui::RichText::new(tr(i18n, "routePlanner.attribution"))
                            .small()
                            .color(theme::TEXT_MUTED),
                    );
                }
            });
        if !open {
            data.route.clear();
            data.route_adding = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{MARKER_SCALE, marker_radius, merge_credits};

    fn merged(credits: &[&str]) -> String {
        merge_credits(&credits.iter().map(|c| c.to_string()).collect::<Vec<_>>())
    }

    #[test]
    fn repeated_parts_across_credits_are_shown_once() {
        assert_eq!(
            merged(&[
                "© OpenStreetMap contributors",
                "© OpenStreetMap contributors | © OpenRailwayMap (CC-BY-SA)"
            ]),
            "© OpenStreetMap contributors  |  © OpenRailwayMap (CC-BY-SA)"
        );
    }

    #[test]
    fn distinct_parts_keep_their_order_and_blanks_are_dropped() {
        assert_eq!(merged(&["B | A", " ", "C", "A"]), "B  |  A  |  C");
        assert_eq!(merged(&[]), "");
    }

    #[test]
    fn marker_radius_is_scaled_from_the_old_sizes() {
        assert!((marker_radius(false) - 6.0 * 1.8).abs() < 1e-4);
        assert!((marker_radius(true) - 8.0 * 1.8).abs() < 1e-4);
        assert!((MARKER_SCALE - 1.8).abs() < 1e-6);
    }
}

/// Screen position of a marker, at the height the renderer draws it: on the
/// terrain `ground` describes, or on the ellipsoid where it knows none.
fn marker_screen_position(
    camera: &terra_globe::Camera,
    point: &terra_core::GeoPoint,
    viewport: (u32, u32),
    ground: &terra_globe::GroundFn<'_>,
) -> Option<terra_globe::ScreenPoint> {
    camera.project(terra_globe::geo_point_to_ecef(point, ground), viewport)
}

#[cfg(test)]
mod marker_height_tests {
    use super::*;

    #[test]
    fn marker_on_terrain_projects_where_the_terrain_draws_it() {
        let mut camera = terra_globe::Camera::default();
        camera.distance = 20_000.0;
        camera.pitch = 40f64.to_radians();
        let target = camera.target_geodetic();
        let point = terra_core::GeoPoint::new(target.lon_deg(), target.lat_deg()).expect("point");
        let viewport = (1280, 800);
        let on_ellipsoid =
            marker_screen_position(&camera, &point, viewport, &terra_globe::no_ground).expect("on");
        let on_mountain =
            marker_screen_position(&camera, &point, viewport, &|_, _| Some(2_500.0)).expect("on");
        assert!(
            on_mountain.y < on_ellipsoid.y - 20.0,
            "2.5 km up must move the marker up the screen: {on_mountain:?} vs {on_ellipsoid:?}"
        );
        // Terrain that knows nothing here leaves the marker on the surface.
        let unknown = marker_screen_position(&camera, &point, viewport, &|_, _| None).expect("on");
        assert_eq!(unknown, on_ellipsoid);
    }

    #[test]
    fn marker_with_altitude_and_no_mode_projects_like_ground_level() {
        let camera = terra_globe::Camera::default();
        let target = camera.target_geodetic();
        let at = |altitude| terra_core::GeoPoint {
            lon: target.lon_deg(),
            lat: target.lat_deg(),
            altitude,
            altitude_mode: None,
        };
        let viewport = (1280, 800);
        let high = marker_screen_position(
            &camera,
            &at(Some(5000.0)),
            viewport,
            &terra_globe::no_ground,
        )
        .expect("high");
        let ground =
            marker_screen_position(&camera, &at(Some(0.0)), viewport, &terra_globe::no_ground)
                .expect("ground");
        assert_eq!(high, ground);
        assert_eq!(
            high,
            marker_screen_position(&camera, &at(None), viewport, &terra_globe::no_ground).unwrap()
        );
    }
}

#[cfg(test)]
mod label_anchor_tests {
    use super::label_anchor;
    use eframe::egui::Align2;
    use terra_core::{GeoPoint, PlacemarkGeometry};

    fn point(lon: f64, lat: f64) -> GeoPoint {
        GeoPoint::new(lon, lat).unwrap()
    }

    #[test]
    fn a_path_is_labelled_above_its_northernmost_point() {
        let ring = PlacemarkGeometry::LineString {
            path: vec![point(9.94, 51.14), point(9.95, 51.15), point(9.96, 51.14)],
            tessellate: Some(true),
            timestamps: None,
        };
        let (anchor, align) = label_anchor(&ring);
        assert_eq!((anchor.lon, anchor.lat), (9.95, 51.15));
        assert_eq!(align, Align2::CENTER_BOTTOM);
    }

    #[test]
    fn concentric_rings_get_distinct_label_anchors() {
        let ring = |radius: f64| PlacemarkGeometry::LineString {
            path: (0..36)
                .map(|step| {
                    let angle = f64::from(step) * 10f64.to_radians();
                    point(9.94 + radius * angle.cos(), 51.14 + radius * angle.sin())
                })
                .collect(),
            tessellate: Some(true),
            timestamps: None,
        };
        let (inner, _) = label_anchor(&ring(0.001));
        let (outer, _) = label_anchor(&ring(0.002));
        assert!(outer.lat - inner.lat > 0.0009);
    }

    #[test]
    fn points_and_empty_paths_keep_the_centre_label() {
        let marker = PlacemarkGeometry::Point {
            coordinates: point(9.94, 51.14),
        };
        assert_eq!(label_anchor(&marker).1, Align2::CENTER_CENTER);
        let empty = PlacemarkGeometry::LineString {
            path: Vec::new(),
            tessellate: None,
            timestamps: None,
        };
        assert_eq!(label_anchor(&empty).1, Align2::CENTER_CENTER);
    }
}

#[cfg(test)]
mod declutter_tests {
    use super::declutter;
    use eframe::egui::{Rect, pos2};

    fn r(x: f32, y: f32) -> Rect {
        Rect::from_min_max(pos2(x, y), pos2(x + 50.0, y + 10.0))
    }

    #[test]
    fn empty_input_keeps_nothing() {
        assert!(declutter(&[], &[]).is_empty());
    }

    #[test]
    fn a_label_covering_another_marker_pin_is_dropped() {
        let pin = Rect::from_center_size(pos2(25.0, 5.0), eframe::egui::Vec2::splat(20.0));
        assert!(declutter(&[(r(0.0, 0.0), 2)], &[pin]).is_empty());
        assert!(declutter(&[(r(0.0, 0.0), 3)], &[pin]).is_empty());
    }

    #[test]
    fn selected_and_hovered_labels_may_cover_pins() {
        let pin = Rect::from_center_size(pos2(25.0, 5.0), eframe::egui::Vec2::splat(20.0));
        assert_eq!(declutter(&[(r(0.0, 0.0), 0)], &[pin]), vec![0]);
        assert_eq!(declutter(&[(r(0.0, 0.0), 1)], &[pin]), vec![0]);
    }

    #[test]
    fn a_pin_clear_of_the_label_does_not_drop_it() {
        let pin = Rect::from_center_size(pos2(25.0, 40.0), eframe::egui::Vec2::splat(20.0));
        assert_eq!(declutter(&[(r(0.0, 0.0), 2)], &[pin]), vec![0]);
    }

    #[test]
    fn separate_labels_are_all_kept() {
        let mut kept = declutter(&[(r(0.0, 0.0), 2), (r(0.0, 100.0), 2)], &[]);
        kept.sort();
        assert_eq!(kept, vec![0, 1]);
    }

    #[test]
    fn overlap_drops_the_lower_priority() {
        assert_eq!(
            declutter(&[(r(0.0, 0.0), 3), (r(10.0, 0.0), 2)], &[]),
            vec![1]
        );
    }

    #[test]
    fn selected_beats_point_beats_path() {
        assert_eq!(
            declutter(
                &[(r(0.0, 0.0), 3), (r(5.0, 0.0), 2), (r(10.0, 0.0), 0)],
                &[]
            ),
            vec![2]
        );
        assert_eq!(
            declutter(&[(r(0.0, 0.0), 3), (r(5.0, 0.0), 2)], &[]),
            vec![1]
        );
    }

    #[test]
    fn equal_priority_overlap_keeps_the_earlier() {
        assert_eq!(
            declutter(&[(r(0.0, 0.0), 2), (r(10.0, 0.0), 2)], &[]),
            vec![0]
        );
    }

    #[test]
    fn labels_just_outside_the_margin_both_stay() {
        let mut kept = declutter(&[(r(0.0, 0.0), 2), (r(52.5, 0.0), 2)], &[]);
        kept.sort();
        assert_eq!(kept, vec![0, 1]);
    }

    #[test]
    fn a_gap_inside_the_margin_still_counts_as_overlap() {
        assert_eq!(
            declutter(&[(r(0.0, 0.0), 2), (r(51.0, 0.0), 2)], &[]),
            vec![0]
        );
    }
}
