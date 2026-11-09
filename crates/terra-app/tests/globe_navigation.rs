//! Globe navigation, driven the way a user drives it: wheel, pointer and keys.
//!
//! These assert the Google Earth semantics the app promises - the wheel zooms
//! without rotating, arrows move along the current heading, shift+arrows rotate
//! and tilt - through the real widget tree rather than the camera API.

#[allow(dead_code)]
mod harness;

use egui_kittest::Harness;
use harness::make_sized;
use terra_globe::CameraPose;

const WINDOW: egui::Vec2 = egui::vec2(1600.0, 1000.0);
/// A point well inside the globe viewport: left panel is 280 wide, the top bar
/// is at the top, and no editor panel is open.
const GLOBE: egui::Pos2 = egui::pos2(900.0, 500.0);

fn pose(
    harness: &Harness,
    data: &std::rc::Rc<std::cell::RefCell<terra_app::AppData>>,
) -> CameraPose {
    let _ = harness;
    data.borrow().globe.pose()
}

fn hover_globe(harness: &mut Harness) {
    harness.hover_at(GLOBE);
    harness.run_steps(2);
}

/// One mouse-wheel notch. Positive `y` is a wheel rolled away from the user,
/// which egui reports as positive scroll.
fn wheel(harness: &mut Harness, y: f32) {
    harness.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: egui::vec2(0.0, y),
        phase: egui::TouchPhase::Move,
        source: egui::MouseWheelSource::Wheel,
        modifiers: egui::Modifiers::default(),
    });
    harness.run_steps(2);
    // The scroll delta is smoothed over a few frames.
    harness.run_steps(2);
    harness.run_steps(2);
}

fn hold_shifted(harness: &mut Harness, key: egui::Key, frames: usize) {
    harness.event(egui::Event::ModifiersChanged(egui::Modifiers {
        shift: true,
        ..egui::Modifiers::default()
    }));
    harness.key_down(key);
    harness.run_steps(frames);
    harness.key_up(key);
    harness.event(egui::Event::ModifiersChanged(egui::Modifiers::default()));
    harness.run_steps(2);
}

#[test]
fn the_wheel_zooms_without_rotating() {
    let Some((mut harness, data)) = make_sized(WINDOW, None) else {
        eprintln!("no GPU adapter; skipping navigation test");
        return;
    };
    hover_globe(&mut harness);
    let before = pose(&harness, &data);

    wheel(&mut harness, 120.0);
    let zoomed_in = pose(&harness, &data);
    assert!(
        zoomed_in.distance < before.distance * 0.95,
        "the wheel must zoom in: {} -> {}",
        before.distance,
        zoomed_in.distance
    );
    assert!(
        (zoomed_in.heading - before.heading).abs() < 1e-9,
        "zooming must not rotate: {before:?} -> {zoomed_in:?}"
    );
    assert!(
        (zoomed_in.pitch - before.pitch).abs() < 1e-9,
        "zooming must not tilt: {before:?} -> {zoomed_in:?}"
    );

    wheel(&mut harness, -240.0);
    let zoomed_out = pose(&harness, &data);
    assert!(
        zoomed_out.distance > zoomed_in.distance * 1.2,
        "the wheel must zoom out: {} -> {}",
        zoomed_in.distance,
        zoomed_out.distance
    );
    assert!((zoomed_out.heading - before.heading).abs() < 1e-9);
    assert!((zoomed_out.pitch - before.pitch).abs() < 1e-9);
}

#[test]
fn arrow_keys_move_along_the_current_view() {
    let Some((mut harness, data)) = make_sized(WINDOW, None) else {
        return;
    };
    hover_globe(&mut harness);
    let before = pose(&harness, &data);
    assert!(before.lat_deg < 80.0, "the default view is not at the pole");

    harness.key_down(egui::Key::ArrowUp);
    harness.run_steps(8);
    harness.key_up(egui::Key::ArrowUp);
    harness.run_steps(2);
    let north = pose(&harness, &data);
    assert!(
        north.lat_deg > before.lat_deg + 0.01,
        "up must move north when north is up: {} -> {}",
        before.lat_deg,
        north.lat_deg
    );
    assert!(
        (north.distance - before.distance).abs() < 1.0,
        "moving must not change the altitude"
    );
    assert!((north.heading - before.heading).abs() < 1e-9);

    harness.key_down(egui::Key::ArrowRight);
    harness.run_steps(8);
    harness.key_up(egui::Key::ArrowRight);
    harness.run_steps(2);
    let east = pose(&harness, &data);
    assert!(
        east.lon_deg > north.lon_deg + 0.01,
        "right must move east when north is up: {} -> {}",
        north.lon_deg,
        east.lon_deg
    );

    harness.key_down(egui::Key::ArrowDown);
    harness.run_steps(8);
    harness.key_up(egui::Key::ArrowDown);
    harness.run_steps(2);
    let back = pose(&harness, &data);
    assert!(
        back.lat_deg < east.lat_deg,
        "down must move south: {} -> {}",
        east.lat_deg,
        back.lat_deg
    );
}

#[test]
fn arrows_follow_the_heading() {
    let Some((mut harness, data)) = make_sized(WINDOW, None) else {
        return;
    };
    hover_globe(&mut harness);
    let before = pose(&harness, &data);

    // Face east, then "forward" has to go east rather than north.
    hold_shifted(&mut harness, egui::Key::ArrowRight, 6);
    let turned = pose(&harness, &data);
    assert!(
        (turned.heading - before.heading).abs() > 0.05,
        "shift+right must rotate: {} -> {}",
        before.heading,
        turned.heading
    );
    assert!(
        (turned.lat_deg - before.lat_deg).abs() < 1e-9
            && (turned.lon_deg - before.lon_deg).abs() < 1e-9,
        "rotating must not move the camera"
    );

    harness.key_down(egui::Key::ArrowUp);
    harness.run_steps(8);
    harness.key_up(egui::Key::ArrowUp);
    harness.run_steps(2);
    let moved = pose(&harness, &data);

    // The movement must point along the screen's up axis, which the compass
    // turned by `heading`: forward = (-sin(heading), cos(heading)) in
    // (east, north) degrees at this latitude.
    let lat = turned.lat_deg.to_radians();
    let east = (moved.lon_deg - turned.lon_deg) * lat.cos();
    let north = moved.lat_deg - turned.lat_deg;
    let length = east.hypot(north);
    assert!(length > 1e-3, "forward must move somewhere");
    let expected = (-turned.heading.sin(), turned.heading.cos());
    let along = (east * expected.0 + north * expected.1) / length;
    assert!(
        along > 0.99,
        "forward must follow the heading {:.2} deg, moved ({east:.3}, {north:.3})",
        turned.heading.to_degrees()
    );
}

#[test]
fn shift_up_and_down_tilt_the_view() {
    let Some((mut harness, data)) = make_sized(WINDOW, None) else {
        return;
    };
    hover_globe(&mut harness);
    let before = pose(&harness, &data);

    // Google Earth: shift+down tilts up towards the horizon.
    hold_shifted(&mut harness, egui::Key::ArrowDown, 4);
    let tilted = pose(&harness, &data);
    assert!(
        tilted.pitch < before.pitch,
        "shift+down must tilt towards the horizon: {} -> {}",
        before.pitch,
        tilted.pitch
    );
    assert!((tilted.distance - before.distance).abs() < 1.0);

    hold_shifted(&mut harness, egui::Key::ArrowUp, 8);
    let upright = pose(&harness, &data);
    assert!(
        upright.pitch > tilted.pitch,
        "shift+up must tilt towards top-down: {} -> {}",
        tilted.pitch,
        upright.pitch
    );
}

#[test]
fn n_and_u_reset_the_orientation() {
    let Some((mut harness, data)) = make_sized(WINDOW, None) else {
        return;
    };
    hover_globe(&mut harness);
    hold_shifted(&mut harness, egui::Key::ArrowRight, 6);
    hold_shifted(&mut harness, egui::Key::ArrowDown, 4);
    let tilted = pose(&harness, &data);
    assert!(tilted.heading.abs() > 0.05 && tilted.pitch < 45f64.to_radians());

    harness.key_press(egui::Key::N);
    harness.run_steps(2);
    assert!(
        pose(&harness, &data).heading.abs() < 1e-6,
        "n must face north"
    );

    harness.key_press(egui::Key::U);
    harness.run_steps(2);
    assert!(
        pose(&harness, &data).pitch > 80f64.to_radians(),
        "u must look straight down"
    );

    let before = pose(&harness, &data);
    harness.key_press(egui::Key::R);
    harness.run_steps(2);
    let after = pose(&harness, &data);
    assert!(after.heading.abs() < 1e-6);
    assert!((after.lon_deg - before.lon_deg).abs() < 1e-6);
}

#[test]
fn dragging_pans_and_shift_dragging_rotates() {
    let Some((mut harness, data)) = make_sized(WINDOW, None) else {
        return;
    };
    hover_globe(&mut harness);
    let before = pose(&harness, &data);

    // Drag right: the globe follows the cursor, so the camera moves west.
    harness.drag_at(GLOBE);
    harness.run_steps(2);
    harness.event(egui::Event::PointerMoved(egui::pos2(
        GLOBE.x + 200.0,
        GLOBE.y,
    )));
    harness.run_steps(2);
    harness.drop_at(egui::pos2(GLOBE.x + 200.0, GLOBE.y));
    harness.run_steps(2);
    let panned = pose(&harness, &data);
    assert!(
        panned.lon_deg < before.lon_deg - 0.01,
        "dragging right must pan west: {} -> {}",
        before.lon_deg,
        panned.lon_deg
    );
    assert!(
        (panned.heading - before.heading).abs() < 1e-9,
        "panning must not rotate"
    );
    assert!((panned.distance - before.distance).abs() < 1.0);

    // Shift+drag rotates instead of panning.
    let before = pose(&harness, &data);
    harness.event(egui::Event::ModifiersChanged(egui::Modifiers {
        shift: true,
        ..egui::Modifiers::default()
    }));
    harness.drag_at(GLOBE);
    harness.run_steps(2);
    harness.event(egui::Event::PointerMoved(egui::pos2(
        GLOBE.x + 150.0,
        GLOBE.y,
    )));
    harness.run_steps(2);
    harness.drop_at(egui::pos2(GLOBE.x + 150.0, GLOBE.y));
    harness.event(egui::Event::ModifiersChanged(egui::Modifiers::default()));
    harness.run_steps(2);
    let rotated = pose(&harness, &data);
    assert!(
        (rotated.heading - before.heading).abs() > 0.05,
        "shift+drag must rotate: {} -> {}",
        before.heading,
        rotated.heading
    );
    assert!(
        (rotated.lon_deg - before.lon_deg).abs() < 1e-6,
        "shift+drag must not pan"
    );
}

mod nav_controls {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;
    use terra_app::AppData;
    use terra_app::views::nav_controls::{NavLayout, last_layout, north_marker};
    use terra_globe::{DrawTool, SceneMode};

    type Data = Rc<RefCell<AppData>>;

    fn nav(harness: &Harness) -> NavLayout {
        last_layout(&harness.ctx).expect("the globe view has rendered")
    }

    fn setup() -> Option<(Harness<'static>, Data)> {
        let (mut harness, data) = make_sized(WINDOW, None)?;
        // Panels settle over a few frames; the globe rect moves until then.
        harness.run_steps(15);
        Some((harness, data))
    }

    fn click(harness: &mut Harness, at: egui::Pos2) {
        harness.hover_at(at);
        harness.run_steps(2);
        harness.drag_at(at);
        harness.run_steps(2);
        harness.drop_at(at);
        harness.run_steps(2);
    }

    fn hold(harness: &mut Harness, at: egui::Pos2, frames: usize) {
        harness.hover_at(at);
        harness.run_steps(2);
        harness.drag_at(at);
        harness.run_steps(frames);
        harness.drop_at(at);
        harness.run_steps(2);
    }

    #[test]
    fn controls_do_not_cover_the_test_point() {
        let Some((harness, _)) = setup() else { return };
        let l = nav(&harness);
        for r in [l.compass, l.move_pad, l.zoom] {
            assert!(!r.contains(GLOBE));
        }
    }

    #[test]
    fn plus_and_minus_zoom_in_and_out() {
        let Some((mut harness, data)) = setup() else {
            return;
        };
        let l = nav(&harness);
        let d0 = pose(&harness, &data).distance;
        click(&mut harness, l.zoom_in.center());
        let d1 = pose(&harness, &data).distance;
        assert!(d1 < d0 * 0.9, "+ must zoom in: {d0} -> {d1}");
        click(&mut harness, l.zoom_out.center());
        click(&mut harness, l.zoom_out.center());
        let d2 = pose(&harness, &data).distance;
        assert!(d2 > d1 * 1.1, "- must zoom out: {d1} -> {d2}");
    }

    #[test]
    fn zoom_knob_zooms_while_held_and_stops_on_release() {
        let Some((mut harness, data)) = setup() else {
            return;
        };
        let l = nav(&harness);
        let d0 = pose(&harness, &data).distance;
        let c = l.zoom_track.center();
        let up = egui::pos2(c.x, l.zoom_track.top() + 4.0);
        hold(&mut harness, up, 10);
        let d1 = pose(&harness, &data).distance;
        assert!(d1 < d0 * 0.95, "knob up must zoom in: {d0} -> {d1}");
        harness.run_steps(10);
        let d2 = pose(&harness, &data).distance;
        assert!(
            (d2 - d1).abs() < 1e-6 * d1,
            "released knob must stop: {d1} -> {d2}"
        );

        let down = egui::pos2(c.x, l.zoom_track.bottom() - 4.0);
        hold(&mut harness, down, 10);
        assert!(pose(&harness, &data).distance > d2 * 1.05);
    }

    #[test]
    fn dragging_the_ring_rotates_and_n_restores_north() {
        let Some((mut harness, data)) = setup() else {
            return;
        };
        let l = nav(&harness);
        let c = l.compass.center();
        let start = egui::pos2(c.x, c.y - 42.0);
        harness.hover_at(start);
        harness.run_steps(2);
        harness.drag_at(start);
        harness.run_steps(2);
        for p in [
            egui::pos2(c.x + 30.0, c.y - 30.0),
            egui::pos2(c.x + 42.0, c.y),
        ] {
            harness.event(egui::Event::PointerMoved(p));
            harness.run_steps(2);
        }
        harness.drop_at(egui::pos2(c.x + 42.0, c.y));
        harness.run_steps(2);
        let h = pose(&harness, &data).heading;
        assert!(h < -0.5, "clockwise drag must carry N clockwise: {h}");

        click(&mut harness, north_marker(l.compass, h));
        assert!(
            pose(&harness, &data).heading.abs() < 1e-6,
            "N must face north"
        );
    }

    #[test]
    fn move_joystick_up_moves_north_without_zooming() {
        let Some((mut harness, data)) = setup() else {
            return;
        };
        let l = nav(&harness);
        let before = pose(&harness, &data);
        let c = l.move_pad.center();
        hold(&mut harness, egui::pos2(c.x, c.y - 28.0), 8);
        let after = pose(&harness, &data);
        assert!(
            after.lat_deg > before.lat_deg + 0.01,
            "{before:?} -> {after:?}"
        );
        assert!((after.distance - before.distance).abs() < 1.0);
        assert!((after.heading - before.heading).abs() < 1e-9);
    }

    #[test]
    fn look_joystick_turns_about_a_fixed_eye() {
        let Some((mut harness, data)) = setup() else {
            return;
        };
        let close = CameraPose {
            distance: 500_000.0,
            ..pose(&harness, &data)
        };
        data.borrow_mut().globe.apply_pose(close);
        harness.run_steps(2);
        let l = nav(&harness);
        let eye = data.borrow().globe.camera.eye_ecef();
        let before = pose(&harness, &data);
        let c = l.look.center();
        hold(&mut harness, egui::pos2(c.x + 24.0, c.y), 10);
        let after = pose(&harness, &data);
        assert!(
            after.heading > before.heading + 0.02,
            "{before:?} -> {after:?}"
        );
        let moved = (data.borrow().globe.camera.eye_ecef() - eye).length();
        assert!(moved < 1.0, "the eye must stay put, moved {moved} m");
    }

    #[test]
    fn clicking_the_controls_does_not_place_markers() {
        let Some((mut harness, data)) = setup() else {
            return;
        };
        data.borrow_mut().drawing.start(DrawTool::Marker);
        let l = nav(&harness);
        let before = data.borrow().store.library().placemarks.len();
        for p in [
            l.zoom_in.center(),
            l.move_pad.center(),
            l.look.center(),
            egui::pos2(l.compass.center().x, l.compass.top() + 6.0),
        ] {
            click(&mut harness, p);
        }
        assert_eq!(data.borrow().store.library().placemarks.len(), before);
        assert!(data.borrow().drawing.is_active());

        // Control: the same click on the globe does add a point.
        click(&mut harness, GLOBE);
        assert_eq!(
            data.borrow().store.library().placemarks.len(),
            before + 1,
            "a click on the globe must reach the drawing tool"
        );
    }

    #[test]
    fn dragging_outside_the_controls_still_pans() {
        let Some((mut harness, data)) = setup() else {
            return;
        };
        hover_globe(&mut harness);
        let before = pose(&harness, &data);
        harness.drag_at(GLOBE);
        harness.run_steps(2);
        harness.event(egui::Event::PointerMoved(egui::pos2(
            GLOBE.x + 150.0,
            GLOBE.y,
        )));
        harness.run_steps(2);
        harness.drop_at(egui::pos2(GLOBE.x + 150.0, GLOBE.y));
        harness.run_steps(2);
        assert!(pose(&harness, &data).lon_deg < before.lon_deg - 0.01);
    }

    #[test]
    fn look_and_rotate_do_nothing_in_2d_but_zoom_and_move_work() {
        let Some((mut harness, data)) = setup() else {
            return;
        };
        data.borrow_mut().set_scene_mode(SceneMode::Scene2D);
        harness.run_steps(3);
        let l = nav(&harness);
        let before = pose(&harness, &data);
        let c = l.look.center();
        hold(&mut harness, egui::pos2(c.x + 24.0, c.y), 6);
        let rc = l.compass.center();
        click(&mut harness, egui::pos2(rc.x, rc.y - 42.0));
        let same = pose(&harness, &data);
        assert!((same.heading - before.heading).abs() < 1e-9);
        assert!((same.lon_deg - before.lon_deg).abs() < 1e-9);
        assert!((same.lat_deg - before.lat_deg).abs() < 1e-9);

        click(&mut harness, l.zoom_in.center());
        assert!(pose(&harness, &data).distance < same.distance);
        let m = l.move_pad.center();
        hold(&mut harness, egui::pos2(m.x, m.y - 28.0), 6);
        assert!(pose(&harness, &data).lat_deg > same.lat_deg);
    }
}
