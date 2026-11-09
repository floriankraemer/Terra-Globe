//! Google-Earth-style navigation overlay at the top-right of the globe: a
//! compass ring around a look joystick, a move joystick and a zoom slider.
//! Everything is painted with the egui painter; the controls own the pointer
//! while it is over them, so the globe underneath never sees those clicks.

use eframe::egui::{self, Color32, Pos2, Rect, Response, Sense, Shape, Stroke, Vec2, pos2, vec2};
use terra_globe::SceneMode;

use super::globe_view::{PAN_RATE, ZOOM_STEP};
use super::tr;
use crate::i18n::I18n;
use crate::{AppData, theme};

const MARGIN: f32 = 12.0;
const COMPASS_D: f32 = 100.0;
const LOOK_D: f32 = 58.0;
const MOVE_D: f32 = 68.0;
const GAP: f32 = 8.0;
const ZOOM_W: f32 = 30.0;
const ZOOM_H: f32 = 128.0;
const NORTH_HIT: f32 = 20.0;

/// Look speed at full joystick deflection, radians per second.
const LOOK_RATE: f64 = 0.8;
/// Zoom steps per second at full slider deflection.
const ZOOM_RATE: f64 = 3.0;
/// A bare click on a joystick still moves a visible amount.
const TAP_SECONDS: f64 = 1.0 / 12.0;

/// Screen rectangles of every control, for hit-testing and for tests.
#[derive(Debug, Clone, Copy)]
pub struct NavLayout {
    /// The whole compass ring, including the area under the look joystick.
    pub compass: Rect,
    pub look: Rect,
    pub move_pad: Rect,
    pub zoom: Rect,
    pub zoom_in: Rect,
    pub zoom_out: Rect,
    /// The slider track between the two buttons; its centre is the rest position.
    pub zoom_track: Rect,
}

pub fn layout(globe: Rect) -> NavLayout {
    let cx = globe.right() - MARGIN - COMPASS_D / 2.0;
    let square = |centre: Pos2, size: Vec2| Rect::from_center_size(centre, size);
    let compass_c = pos2(cx, globe.top() + MARGIN + COMPASS_D / 2.0);
    let move_c = pos2(cx, compass_c.y + COMPASS_D / 2.0 + GAP + MOVE_D / 2.0);
    let zoom_top = move_c.y + MOVE_D / 2.0 + GAP;
    let zoom = Rect::from_min_size(pos2(cx - ZOOM_W / 2.0, zoom_top), vec2(ZOOM_W, ZOOM_H));
    let button = vec2(ZOOM_W, ZOOM_W);
    NavLayout {
        compass: square(compass_c, Vec2::splat(COMPASS_D)),
        look: square(compass_c, Vec2::splat(LOOK_D)),
        move_pad: square(move_c, Vec2::splat(MOVE_D)),
        zoom,
        zoom_in: Rect::from_min_size(zoom.min, button),
        zoom_out: Rect::from_min_size(pos2(zoom.min.x, zoom.max.y - ZOOM_W), button),
        zoom_track: Rect::from_min_max(
            pos2(zoom.min.x, zoom.min.y + ZOOM_W),
            pos2(zoom.max.x, zoom.max.y - ZOOM_W),
        ),
    }
}

/// Pointer offset from a joystick centre, normalised so the rim is length 1
/// and clamped to the unit disc. A degenerate radius gives zero.
pub fn joystick_vector(center: Pos2, radius: f32, pointer: Pos2) -> Vec2 {
    if radius.is_nan() || radius <= 0.0 {
        return Vec2::ZERO;
    }
    let v = (pointer - center) / radius;
    if !v.x.is_finite() || !v.y.is_finite() {
        return Vec2::ZERO;
    }
    if v.length() > 1.0 { v.normalized() } else { v }
}

fn layout_key() -> egui::Id {
    egui::Id::NULL.with("nav_controls_layout")
}

/// The layout used on the latest frame, so tests can aim at the real rects.
pub fn last_layout(ctx: &egui::Context) -> Option<NavLayout> {
    ctx.data(|d| d.get_temp(layout_key()))
}

/// Where the compass's "N" sits on the ring for a given camera heading.
pub fn north_marker(compass: Rect, heading: f64) -> Pos2 {
    let angle = -heading as f32;
    compass.center() + (COMPASS_D / 2.0 - 9.0) * vec2(angle.sin(), -angle.cos())
}

/// Which controls are hovered or in use, for painting.
#[derive(Debug, Default, Clone, Copy)]
pub struct NavState {
    hot_compass: bool,
    hot_look: bool,
    hot_move: bool,
    hot_in: bool,
    hot_out: bool,
    hot_north: bool,
    hot_track: bool,
    /// Slider deflection, `1` = fully up (zooming in); `0` when released.
    knob: f32,
}

fn engaged(response: &Response, dt: f64) -> Option<f64> {
    if response.is_pointer_button_down_on() {
        Some(dt)
    } else if response.clicked() {
        Some(dt.max(TAP_SECONDS))
    } else {
        None
    }
}

fn pointer_angle(centre: Pos2, p: Pos2) -> f32 {
    (p.x - centre.x).atan2(centre.y - p.y)
}

/// Runs the controls' input and drives the camera. Call after the globe's own
/// response is allocated and before the globe handles input, so the controls
/// sit on top of it in egui's hit test.
pub(super) fn interact(
    data: &mut AppData,
    i18n: &I18n,
    ui: &egui::Ui,
    globe_rect: Rect,
) -> NavState {
    let l = layout(globe_rect);
    ui.ctx().data_mut(|d| d.insert_temp(layout_key(), l));
    let mode = data.globe.mode();
    let can_look = mode == SceneMode::Scene3D;
    let can_rotate = mode != SceneMode::Scene2D;
    let dt = f64::from(ui.input(|i| i.stable_dt)).clamp(0.001, 0.1);
    let id = ui.scope_id().with("nav_controls");
    let drag = Sense::click_and_drag();
    let heading = data.globe.pose().heading;

    let compass = ui.interact(l.compass, id.with("compass"), drag);
    let look = ui.interact(l.look, id.with("look"), drag);
    let mv = ui.interact(l.move_pad, id.with("move"), drag);
    let zoom_in = ui.interact(l.zoom_in, id.with("zoom_in"), Sense::click());
    let zoom_out = ui.interact(l.zoom_out, id.with("zoom_out"), Sense::click());
    let track = ui.interact(l.zoom_track, id.with("zoom_track"), drag);
    let north_rect =
        Rect::from_center_size(north_marker(l.compass, heading), Vec2::splat(NORTH_HIT));
    let north = ui.interact(north_rect, id.with("north"), Sense::click());

    compass.clone().on_hover_text(tr(i18n, "nav.compass"));
    look.clone().on_hover_text(tr(i18n, "nav.look"));
    mv.clone().on_hover_text(tr(i18n, "nav.move"));
    track.clone().on_hover_text(tr(i18n, "nav.zoom"));

    let mut busy = false;

    if can_rotate && compass.dragged_by(egui::PointerButton::Primary) {
        if let Some(p) = compass.interact_pointer_pos() {
            let centre = l.compass.center();
            if (p - centre).length() > 4.0 {
                let before = pointer_angle(centre, p - compass.drag_delta());
                let delta = (pointer_angle(centre, p) - before + std::f32::consts::PI)
                    .rem_euclid(std::f32::consts::TAU)
                    - std::f32::consts::PI;
                // Dragging the ring clockwise carries the N marker with it.
                data.globe.rotate_by(-f64::from(delta));
                busy = true;
            }
        }
    }
    if can_rotate && north.clicked() {
        data.globe.set_north_up();
    }

    if can_look
        && let Some(step) = engaged(&look, dt)
        && let Some(p) = look.interact_pointer_pos()
    {
        let v = joystick_vector(l.look.center(), l.look.width() / 2.0, p);
        data.globe.look_by(
            f64::from(v.x) * LOOK_RATE * step,
            f64::from(v.y) * LOOK_RATE * step,
        );
        busy = true;
    }

    if let Some(step) = engaged(&mv, dt)
        && let Some(p) = mv.interact_pointer_pos()
    {
        let v = joystick_vector(l.move_pad.center(), l.move_pad.width() / 2.0, p);
        let metres = data.globe.view_distance() * PAN_RATE * step;
        data.globe
            .move_view(f64::from(-v.y) * metres, f64::from(v.x) * metres);
        busy = true;
    }

    if zoom_in.clicked() {
        data.globe.zoom_by(ZOOM_STEP);
    }
    if zoom_out.clicked() {
        data.globe.zoom_by(1.0 / ZOOM_STEP);
    }

    let mut knob = 0.0;
    if track.is_pointer_button_down_on()
        && let Some(p) = track.interact_pointer_pos()
    {
        let half = l.zoom_track.height() / 2.0;
        knob = ((l.zoom_track.center().y - p.y) / half).clamp(-1.0, 1.0);
        data.globe
            .zoom_by(ZOOM_STEP.powf(ZOOM_RATE * f64::from(knob) * dt));
        busy = true;
    }

    if busy {
        ui.ctx().request_repaint();
    }
    NavState {
        hot_compass: can_rotate && (compass.hovered() || compass.dragged()),
        hot_look: can_look && (look.hovered() || look.is_pointer_button_down_on()),
        hot_move: mv.hovered() || mv.is_pointer_button_down_on(),
        hot_in: zoom_in.hovered(),
        hot_out: zoom_out.hovered(),
        hot_north: can_rotate && north.hovered(),
        hot_track: track.hovered() || track.is_pointer_button_down_on(),
        knob,
    }
}

fn blue() -> Color32 {
    theme::ACCENT.lerp_to_gamma(Color32::BLACK, 0.35)
}

fn alpha(hot: bool, enabled: bool) -> f32 {
    match (enabled, hot) {
        (false, _) => 0.3,
        (true, true) => 1.0,
        (true, false) => 0.6,
    }
}

fn disc(painter: &egui::Painter, c: Pos2, r: f32, a: f32) {
    painter.circle(
        c,
        r,
        Color32::from_gray(205).gamma_multiply(a),
        Stroke::new(1.0, Color32::from_gray(120).gamma_multiply(a)),
    );
    painter.circle_filled(
        c - vec2(0.0, r * 0.22),
        r * 0.72,
        Color32::from_gray(242).gamma_multiply(a),
    );
}

fn arrow(painter: &egui::Painter, c: Pos2, dir: Vec2, size: f32, color: Color32) {
    let perp = vec2(-dir.y, dir.x);
    painter.add(Shape::convex_polygon(
        vec![
            c + dir * size,
            c - dir * size * 0.6 + perp * size * 0.9,
            c - dir * size * 0.6 - perp * size * 0.9,
        ],
        color,
        Stroke::NONE,
    ));
}

fn four_arrows(painter: &egui::Painter, c: Pos2, radius: f32, color: Color32) {
    for dir in [
        vec2(0.0, -1.0),
        vec2(1.0, 0.0),
        vec2(0.0, 1.0),
        vec2(-1.0, 0.0),
    ] {
        arrow(painter, c + dir * radius, dir, 4.5, color);
    }
}

pub(super) fn paint(ui: &egui::Ui, globe_rect: Rect, data: &AppData, state: &NavState) {
    let painter = ui.painter().with_clip_rect(globe_rect);
    let l = layout(globe_rect);
    let mode = data.globe.mode();
    let can_look = mode == SceneMode::Scene3D;
    let can_rotate = mode != SceneMode::Scene2D;
    let a = |hot, enabled| alpha(hot, enabled);

    // Compass ring with the rotating N marker.
    let cc = l.compass.center();
    let ring_a = a(state.hot_compass, can_rotate);
    disc(&painter, cc, COMPASS_D / 2.0, ring_a);
    let heading = if can_rotate {
        data.globe.pose().heading
    } else {
        0.0
    };
    let n = north_marker(l.compass, heading);
    let n_a = a(state.hot_north || state.hot_compass, can_rotate);
    painter.circle_filled(n, 8.0, blue().gamma_multiply(n_a));
    painter.text(
        n,
        egui::Align2::CENTER_CENTER,
        "N",
        egui::FontId::proportional(11.0),
        Color32::WHITE.gamma_multiply(n_a),
    );

    // Look joystick: an eye inside four arrows.
    let look_a = a(state.hot_look, can_look);
    let lc = l.look.center();
    let lr = LOOK_D / 2.0;
    disc(&painter, lc, lr, look_a);
    let col = blue().gamma_multiply(look_a);
    four_arrows(&painter, lc, lr * 0.72, col);
    let eye: Vec<Pos2> = (0..16)
        .map(|i| {
            let t = i as f32 / 16.0 * std::f32::consts::TAU;
            lc + vec2(t.cos() * 9.0, t.sin() * 5.0)
        })
        .collect();
    painter.add(Shape::convex_polygon(
        eye,
        Color32::WHITE.gamma_multiply(look_a),
        Stroke::new(1.5, col),
    ));
    painter.circle_filled(lc, 3.0, col);

    // Move joystick: a hand inside four arrows.
    let move_a = a(state.hot_move, true);
    let mc = l.move_pad.center();
    let mr = MOVE_D / 2.0;
    disc(&painter, mc, mr, move_a);
    let col = blue().gamma_multiply(move_a);
    four_arrows(&painter, mc, mr * 0.76, col);
    painter.rect_filled(
        Rect::from_center_size(mc + vec2(0.0, 3.0), vec2(11.0, 8.0)),
        2.0,
        col,
    );
    for i in 0..4 {
        let x = mc.x - 4.5 + i as f32 * 3.0;
        painter.line_segment([pos2(x, mc.y), pos2(x, mc.y - 7.0)], Stroke::new(2.0, col));
    }
    painter.line_segment(
        [pos2(mc.x - 7.0, mc.y + 3.0), pos2(mc.x - 9.0, mc.y - 1.0)],
        Stroke::new(2.0, col),
    );

    // Zoom slider: + on top, − at the bottom, a knob that springs back.
    let track_a = a(state.hot_track, true);
    let track = l.zoom_track;
    painter.rect(
        Rect::from_center_size(track.center(), vec2(8.0, track.height() - 6.0)),
        4.0,
        Color32::from_gray(205).gamma_multiply(track_a),
        Stroke::new(1.0, Color32::from_gray(120).gamma_multiply(track_a)),
        egui::StrokeKind::Inside,
    );
    for (rect, hot, plus) in [
        (l.zoom_in, state.hot_in, true),
        (l.zoom_out, state.hot_out, false),
    ] {
        let ba = a(hot, true);
        let c = rect.center();
        disc(&painter, c, ZOOM_W / 2.0, ba);
        let stroke = Stroke::new(2.5, blue().gamma_multiply(ba));
        painter.line_segment([c - vec2(5.0, 0.0), c + vec2(5.0, 0.0)], stroke);
        if plus {
            painter.line_segment([c - vec2(0.0, 5.0), c + vec2(0.0, 5.0)], stroke);
        }
    }
    let knob_c = track.center() - vec2(0.0, state.knob * track.height() / 2.0);
    disc(&painter, knob_c, 8.0, track_a);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joystick_is_zero_at_the_centre() {
        let c = pos2(10.0, 20.0);
        assert_eq!(joystick_vector(c, 30.0, c), Vec2::ZERO);
    }

    #[test]
    fn joystick_is_unit_at_the_rim_and_clamped_beyond() {
        let c = pos2(0.0, 0.0);
        assert_eq!(joystick_vector(c, 30.0, pos2(30.0, 0.0)), vec2(1.0, 0.0));
        let far = joystick_vector(c, 30.0, pos2(300.0, 400.0));
        assert!((far.length() - 1.0).abs() < 1e-6);
        assert!((far.x - 0.6).abs() < 1e-6 && (far.y - 0.8).abs() < 1e-6);
        assert_eq!(joystick_vector(c, 30.0, pos2(15.0, 0.0)), vec2(0.5, 0.0));
    }

    #[test]
    fn joystick_never_yields_nan() {
        let c = pos2(1.0, 1.0);
        for r in [0.0, -5.0, f32::NAN] {
            assert_eq!(joystick_vector(c, r, pos2(9.0, 9.0)), Vec2::ZERO);
            assert_eq!(joystick_vector(c, r, c), Vec2::ZERO);
        }
        assert_eq!(joystick_vector(c, 10.0, pos2(f32::NAN, 0.0)), Vec2::ZERO);
    }

    #[test]
    fn layout_is_top_right_inside_the_globe_without_overlap() {
        let globe = Rect::from_min_size(pos2(280.0, 40.0), vec2(1200.0, 900.0));
        let l = layout(globe);
        let blocks = [l.compass, l.move_pad, l.zoom];
        for r in blocks {
            assert!(globe.contains_rect(r), "{r:?} outside {globe:?}");
        }
        assert!(l.compass.top() >= globe.top() + MARGIN - 0.01);
        assert!((globe.right() - l.compass.right() - MARGIN).abs() < 0.01);
        for (i, a) in blocks.iter().enumerate() {
            for b in &blocks[i + 1..] {
                assert!(!a.intersects(*b), "{a:?} overlaps {b:?}");
            }
        }
        assert!(l.compass.contains_rect(l.look));
        assert!(l.zoom.contains_rect(l.zoom_in) && l.zoom.contains_rect(l.zoom_out));
        assert!(
            l.zoom_in.bottom() <= l.zoom_track.top() && l.zoom_track.bottom() <= l.zoom_out.top()
        );
        assert!(l.zoom_in.top() < l.zoom_out.top());
    }

    #[test]
    fn north_marker_sits_on_top_at_heading_zero_and_moves_with_heading() {
        let r = Rect::from_center_size(pos2(0.0, 0.0), Vec2::splat(100.0));
        let n = north_marker(r, 0.0);
        assert!(n.x.abs() < 1e-4 && n.y < -30.0);
        // Heading east of north puts north on the left of the screen.
        assert!(north_marker(r, std::f64::consts::FRAC_PI_2).x < -30.0);
    }
}
