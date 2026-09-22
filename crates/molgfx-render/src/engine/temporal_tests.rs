use super::{IllustrationStyle, TemporalOptions, TemporalState};
use crate::{BackdropStyle, DisplayTransform};
use molgfx_math::{Camera, Mat4, Projection, Vec3};

fn camera() -> Camera {
    Camera {
        eye: Vec3::new(0.0, 0.0, 8.0),
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

fn options(quality: bool) -> TemporalOptions {
    TemporalOptions {
        extent: [800, 800],
        reset: false,
        quality,
        publication: false,
        illustration: IllustrationStyle::default(),
        optics: [8.0, 0.0, 0.0, 0.0],
        motion_blur: [0.0; 4],
        atmosphere: crate::engine::backdrop::pack(
            BackdropStyle::default(),
            DisplayTransform::default(),
            false,
            [0.0; 4],
        ),
        lighting: crate::LightingEnvironment::default().packed(),
        shadow_view: Mat4::IDENTITY,
        shadow_projection: Mat4::IDENTITY,
        shadow_view_proj: Mat4::IDENTITY,
    }
}

#[test]
fn the_first_sample_rejects_history_and_the_second_reprojects_it() {
    let mut state = TemporalState::default();
    let first = state.prepare(&camera(), &options(false));
    let second = state.prepare(&camera(), &options(false));
    assert_eq!(first.temporal[0].to_bits(), 0.0f32.to_bits());
    assert_eq!(second.temporal[0].to_bits(), 1.0f32.to_bits());
    assert_ne!(first.proj, second.proj);
    assert_eq!(state.write_index(), 1);
}

#[test]
fn a_camera_cut_discards_the_previous_history() {
    let mut state = TemporalState::default();
    let _ = state.prepare(&camera(), &options(false));
    let mut cut = camera();
    cut.eye = Vec3::new(20.0, 0.0, 8.0);
    let after_cut = state.prepare(&cut, &options(false));
    assert_eq!(after_cut.temporal[0].to_bits(), 0.0f32.to_bits());
    assert_eq!(after_cut.temporal[3].to_bits(), 0.0f32.to_bits());
}

#[test]
fn any_camera_motion_blocks_quality_until_a_stable_frame() {
    let mut state = TemporalState::default();
    assert!(state.camera_changed(&camera()));
    let _ = state.prepare(&camera(), &options(true));
    assert!(!state.camera_changed(&camera()));
    let mut moved = camera();
    moved.target.x = 0.01;
    assert!(state.camera_changed(&moved));
}

#[test]
fn caller_scheduling_stops_after_each_mode_reaches_its_sample_budget() {
    let mut realtime = TemporalState::default();
    for _ in 0..8 {
        let _ = realtime.prepare(&camera(), &options(false));
    }
    assert!(!realtime.needs_another_frame(8));

    let mut quality = TemporalState::default();
    for _ in 0..63 {
        let _ = quality.prepare(&camera(), &options(true));
    }
    assert!(quality.needs_another_frame(64));
    let _ = quality.prepare(&camera(), &options(true));
    assert!(!quality.needs_another_frame(64));
}

#[test]
fn a_small_camera_move_restarts_refinement_without_discarding_reprojectable_history() {
    let mut state = TemporalState::default();
    for _ in 0..8 {
        state.prepare(&camera(), &options(false));
    }
    let mut moved = camera();
    moved.eye.x += 0.01;
    let first = state.prepare(&moved, &options(false));
    assert_ne!(first.temporal[0].to_bits(), 0.0_f32.to_bits());
    assert!(state.needs_another_frame(8));
    for _ in 0..7 {
        state.prepare(&moved, &options(false));
    }
    assert!(!state.needs_another_frame(8));
    state.invalidate_convergence();
    assert!(state.needs_another_frame(8));
}
