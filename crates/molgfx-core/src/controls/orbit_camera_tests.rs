use super::*;
use molgfx_math::{Aabb, Camera, Projection, Vec3};

fn start() -> OrbitCamera {
    let bounds = Aabb::new(Vec3::new(-10.0, -10.0, -10.0), Vec3::new(10.0, 10.0, 10.0));
    OrbitCamera::new(Camera::framing_aabb(&bounds, 1.5))
}

#[test]
fn orbiting_keeps_the_distance_to_the_focus_point() {
    let mut cam = start();
    let before = cam.distance();
    cam.orbit(120.0, -45.0);
    assert!((cam.distance() - before).abs() < 1e-3);
}

#[test]
fn orbiting_moves_the_eye_but_not_the_target() {
    let mut cam = start();
    let (eye, target) = (cam.camera().eye, cam.camera().target);
    cam.orbit(60.0, 0.0);
    assert_ne!(cam.camera().eye, eye);
    assert_eq!(cam.camera().target, target);
}

#[test]
fn zooming_in_shrinks_the_distance_and_never_reaches_zero() {
    let mut cam = start();
    let before = cam.distance();
    cam.zoom(2.0);
    assert!(cam.distance() < before);
    cam.zoom(10_000.0);
    assert!(cam.distance() >= 0.05);
}

#[test]
fn panning_moves_eye_and_target_together() {
    let mut cam = start();
    let before = cam.distance();
    cam.pan(40.0, 25.0);
    assert!((cam.distance() - before).abs() < 1e-3);
    assert_ne!(cam.camera().target, Vec3::new(0.0, 0.0, 0.0));
}

#[test]
fn reset_returns_to_the_framing_camera() {
    let mut cam = start();
    let home = *cam.camera();
    cam.orbit(80.0, 30.0);
    cam.zoom(3.0);
    cam.pan(10.0, 10.0);
    cam.reset();
    assert_eq!(*cam.camera(), home);
}

#[test]
fn a_non_finite_aspect_is_ignored() {
    let mut cam = start();
    let before = *cam.camera();
    cam.set_aspect(f32::NAN);
    cam.set_aspect(0.0);
    assert_eq!(*cam.camera(), before);
}

fn near_far(c: &OrbitCamera) -> (f32, f32) {
    match c.camera().projection {
        molgfx_math::Projection::Perspective { near, far, .. }
        | molgfx_math::Projection::OffAxis { near, far, .. }
        | molgfx_math::Projection::Orthographic { near, far, .. } => (near, far),
    }
}

#[test]
fn zooming_in_keeps_the_focus_point_in_front_of_the_near_plane() {
    // Regression: the near plane stayed at the framing distance, so as the eye
    // approached, the molecule was clipped away piece by piece.
    let mut cam = start();
    for _ in 0..40 {
        cam.zoom(1.0);
        let (near, _) = near_far(&cam);
        assert!(
            near < cam.distance(),
            "near {near} is past the focus at {}",
            cam.distance()
        );
    }
}

#[test]
fn the_depth_range_always_brackets_the_whole_scene() {
    let mut cam = start();
    let radius = 10.0 * 3.0_f32.sqrt();
    for step in 0..60 {
        cam.zoom(if step % 3 == 0 { -2.0 } else { 1.5 });
        cam.orbit(25.0, 10.0);
        let (near, far) = near_far(&cam);
        // the far side of the scene is at most distance + radius from the eye
        assert!(
            far >= cam.distance() + radius * 0.9,
            "far {far} cuts the back of the scene"
        );
        assert!(near > 0.0 && near < far);
    }
}

#[test]
fn zooming_out_pushes_the_far_plane_out_with_it() {
    let mut cam = start();
    let (_, far_before) = near_far(&cam);
    cam.zoom(-10.0);
    let (_, far_after) = near_far(&cam);
    assert!(far_after > far_before);
}

#[test]
fn reset_restores_the_framing_depth_range_too() {
    let mut cam = start();
    let before = near_far(&cam);
    cam.zoom(15.0);
    cam.reset();
    let after = near_far(&cam);
    assert!((before.0 - after.0).abs() < 1.0 && (before.1 - after.1).abs() < 1.0);
}

#[test]
fn a_saved_pose_can_be_returned_to_after_moving() {
    let mut cam = start();
    let (eye, target, up) = (cam.camera().eye, cam.camera().target, cam.camera().up);
    cam.orbit(80.0, 30.0);
    cam.zoom(3.0);
    assert_ne!(cam.camera().eye, eye);
    cam.set_pose(eye, target, up);
    assert_eq!(
        (cam.camera().eye, cam.camera().target, cam.camera().up),
        (eye, target, up)
    );
}

#[test]
fn an_orthographic_view_keeps_what_the_focus_plane_shows_and_zooming_still_scales() {
    let mut cam = start();
    let Projection::Perspective { fov_y, .. } = cam.camera().projection else {
        panic!("the framing camera is perspective");
    };
    let shown = |c: &OrbitCamera| 2.0 * c.distance() * (fov_y * 0.5).tan();
    cam.set_orthographic(true);
    assert!(cam.is_orthographic());
    let Projection::Orthographic { height, .. } = cam.camera().projection else {
        panic!("orthographic expected");
    };
    assert!((height - shown(&cam)).abs() < 1e-3);
    cam.zoom(3.0);
    let Projection::Orthographic { height: after, .. } = cam.camera().projection else {
        panic!("still orthographic");
    };
    assert!(after < height, "zooming in shows less");
    cam.orbit(30.0, 10.0);
    assert!(matches!(
        cam.camera().projection,
        Projection::Orthographic { .. }
    ));
}

#[test]
fn going_back_to_perspective_restores_the_field_of_view_and_reset_keeps_the_choice() {
    let mut cam = start();
    let Projection::Perspective { fov_y, .. } = cam.camera().projection else {
        panic!();
    };
    cam.set_orthographic(true);
    cam.reset();
    assert!(matches!(
        cam.camera().projection,
        Projection::Orthographic { .. }
    ));
    cam.set_orthographic(false);
    let Projection::Perspective { fov_y: back, .. } = cam.camera().projection else {
        panic!("perspective expected");
    };
    assert!((back - fov_y).abs() < 1e-6);
}

#[test]
fn rolling_turns_the_up_vector_about_the_line_of_sight_and_nothing_else() {
    let mut cam = start();
    let (eye, target) = (cam.camera().eye, cam.camera().target);
    let up = cam.camera().up;
    cam.roll(std::f32::consts::FRAC_PI_2);
    assert_eq!(cam.camera().eye, eye);
    assert_eq!(cam.camera().target, target);
    let rolled = cam.camera().up;
    assert!(
        rolled.dot(up).abs() < 1e-4,
        "a quarter turn is perpendicular"
    );
    assert!((rolled.length() - 1.0).abs() < 1e-4);
    // and back
    cam.roll(-std::f32::consts::FRAC_PI_2);
    assert!(cam.camera().up.distance(up) < 1e-4);
    cam.roll(f32::NAN);
    assert!(cam.camera().up.distance(up) < 1e-4);
}

#[test]
fn a_pinch_factor_scales_the_distance_and_a_bad_one_is_ignored() {
    let mut cam = start();
    let d = cam.distance();
    cam.zoom_by(2.0);
    assert!((cam.distance() - d / 2.0).abs() < 1e-3);
    cam.zoom_by(0.0);
    cam.zoom_by(f32::NAN);
    assert!((cam.distance() - d / 2.0).abs() < 1e-3);
}

fn planes(c: &OrbitCamera) -> (f32, f32) {
    match c.camera().projection {
        Projection::Perspective { near, far, .. }
        | Projection::OffAxis { near, far, .. }
        | Projection::Orthographic { near, far, .. } => (near, far),
    }
}

#[test]
fn clips_push_the_near_plane_in_and_the_far_plane_out_and_zero_puts_them_back() {
    let mut cam = start();
    cam.set_clip(0.0, 0.0);
    let (near, far) = planes(&cam);
    cam.set_clip(4.0, 3.0);
    let (n, f) = planes(&cam);
    assert!((n - (near + 4.0)).abs() < 1e-3, "{n} vs {near}");
    assert!((f - (far - 3.0)).abs() < 1e-3, "{f} vs {far}");
    cam.set_clip(0.0, 0.0);
    assert_eq!(planes(&cam), (near, far));
}

#[test]
fn the_clips_can_never_leave_less_than_a_sliver_or_cross() {
    let mut cam = start();
    cam.set_clip(10_000.0, 10_000.0);
    let (n, f) = planes(&cam);
    assert!(f - n >= 0.5 - 1e-4 && n > 0.0, "{n} {f}");
    cam.set_clip(f32::NAN, -5.0);
    assert_eq!(cam.clip(), (0.0, 0.0));
}

#[test]
fn a_clip_follows_the_camera_and_survives_reset_and_orthographic() {
    let mut cam = start();
    let base = planes(&cam);
    cam.set_clip(3.0, 0.0);
    cam.orbit(40.0, 10.0);
    cam.zoom(1.0);
    let (n, f) = planes(&cam);
    // still clipped: the near plane is deeper than the unclipped fit for this eye
    cam.set_clip(0.0, 0.0);
    let (n0, f0) = planes(&cam);
    assert!(n >= n0 && (f - f0).abs() < 1e-3);
    cam.set_clip(3.0, 0.0);
    cam.reset();
    assert!(planes(&cam).0 > base.0);
    cam.set_orthographic(true);
    assert!(planes(&cam).0 > base.0 - 1e-3);
    assert!(cam.depth() > 0.0);
}

#[test]
fn refocusing_moves_the_focus_and_keeps_the_direction_the_molecule_is_seen_from() {
    let mut cam = start();
    cam.orbit(60.0, 25.0);
    let (eye, target) = (cam.camera().eye, cam.camera().target);
    let direction = (eye - target).normalize();
    let up = cam.camera().up;
    let other = Aabb::new(Vec3::new(490.0, -5.0, -5.0), Vec3::new(510.0, 5.0, 5.0));
    let framing = Camera::framing_aabb(&other, 1.5);
    cam.refocus(framing);
    assert!((cam.camera().target - framing.target).length() < 1e-3);
    let now = (cam.camera().eye - cam.camera().target).normalize();
    assert!(now.dot(direction) > 0.9999, "{now:?} vs {direction:?}");
    assert!(cam.camera().up.dot(up) > 0.9999);
    // and the depth range now brackets the new place, not the old one
    let (near, far) = planes(&cam);
    let d = cam.distance();
    assert!(near < d && far > d, "{near} {d} {far}");
    // reset goes home to the new framing
    cam.orbit(10.0, 10.0);
    cam.reset();
    assert!((cam.camera().target - framing.target).length() < 1e-3);
}

#[test]
fn a_camera_is_at_its_framing_until_it_is_zoomed_or_panned_but_not_when_turned() {
    let mut cam = start();
    assert!(cam.at_framing());
    cam.orbit(50.0, 10.0);
    assert!(cam.at_framing());
    cam.zoom(2.0);
    assert!(!cam.at_framing());
    let mut panned = start();
    panned.pan(40.0, 25.0);
    assert!(!panned.at_framing());
}
