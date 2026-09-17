use super::*;
use molgfx_math::{Camera, Projection, Vec3};

fn camera() -> Camera {
    Camera {
        eye: Vec3::new(0.0, 0.0, 10.0),
        target: Vec3::ZERO,
        up: Vec3::Y,
        projection: Projection::Perspective {
            fov_y: 0.8,
            aspect: 1.0,
            near: 0.1,
            far: 100.0,
        },
    }
}

fn drag(controller: &mut ArcballController, camera: &mut Camera, from: (f32, f32), to: (f32, f32)) {
    controller.update(
        InputEvent::PointerButton {
            button: Button::Left,
            pressed: true,
            x: from.0,
            y: from.1,
        },
        camera,
    );
    controller.update(
        InputEvent::PointerMove {
            x: from.0,
            y: from.1,
        },
        camera,
    );
    controller.update(InputEvent::PointerMove { x: to.0, y: to.1 }, camera);
    controller.update(
        InputEvent::PointerButton {
            button: Button::Left,
            pressed: false,
            x: to.0,
            y: to.1,
        },
        camera,
    );
}

#[test]
fn an_arcball_drag_orbits_the_eye_at_constant_distance() {
    let mut cam = camera();
    let mut ctl = ArcballController::default();
    let before = cam.focus_distance();
    drag(&mut ctl, &mut cam, (100.0, 100.0), (160.0, 130.0));
    assert!(
        (cam.focus_distance() - before).abs() < 1e-3,
        "orbit keeps radius"
    );
    assert!(
        cam.eye.distance(Vec3::new(0.0, 0.0, 10.0)) > 0.1,
        "the eye moved"
    );
    assert_eq!(cam.target, Vec3::ZERO, "the target stays put");
}

#[test]
fn scrolling_zooms_toward_the_target_without_crossing_it() {
    let mut cam = camera();
    let mut ctl = ArcballController::default();
    let before = cam.focus_distance();
    ctl.update(InputEvent::Scroll { delta: 2.0 }, &mut cam);
    assert!(cam.focus_distance() < before);
    for _ in 0..200 {
        ctl.update(InputEvent::Scroll { delta: 5.0 }, &mut cam);
    }
    assert!(cam.focus_distance() > 0.0, "zoom clamps before the target");
}

#[test]
fn an_orbit_drag_keeps_the_horizon_level() {
    let mut cam = camera();
    let mut ctl = OrbitController::default();
    ctl.update(
        InputEvent::PointerButton {
            button: Button::Left,
            pressed: true,
            x: 0.0,
            y: 0.0,
        },
        &mut cam,
    );
    ctl.update(InputEvent::PointerMove { x: 0.0, y: 0.0 }, &mut cam);
    ctl.update(InputEvent::PointerMove { x: 400.0, y: 250.0 }, &mut cam);
    assert_eq!(cam.up, Vec3::Y, "orbit never tilts the horizon");
    assert!((cam.focus_distance() - 10.0).abs() < 1e-3);
}

#[test]
fn flying_forward_moves_eye_and_target_together() {
    let mut cam = camera();
    let mut ctl = FlyController::default();
    ctl.update(
        InputEvent::Key {
            key: Key::Forward,
            pressed: true,
        },
        &mut cam,
    );
    let gap_before = cam.focus_distance();
    ctl.advance(&mut cam, 0.5, 10.0);
    assert!(cam.eye.z < 10.0, "moved toward the scene");
    assert!((cam.focus_distance() - gap_before).abs() < 1e-4);
}

#[test]
fn a_right_button_drag_pans_eye_and_target_in_lockstep() {
    let mut cam = camera();
    let mut ctl = ArcballController::default();
    ctl.update(
        InputEvent::PointerButton {
            button: Button::Right,
            pressed: true,
            x: 0.0,
            y: 0.0,
        },
        &mut cam,
    );
    ctl.update(InputEvent::PointerMove { x: 0.0, y: 0.0 }, &mut cam);
    ctl.update(InputEvent::PointerMove { x: 50.0, y: -20.0 }, &mut cam);
    assert_ne!(cam.target, Vec3::ZERO, "pan moves the target");
    let offset = cam.eye - cam.target;
    assert!(
        (offset.length() - 10.0).abs() < 1e-3,
        "pan preserves the offset"
    );
}
