use super::tests::{camera, engine};
use super::*;
use molgfx_core::Scene;

#[test]
fn completion_keeps_submitted_settings_when_the_next_output_changes() {
    let mut engine = engine();
    let scene = Scene::new();
    let first = engine.render(&scene, &camera()).unwrap();
    assert_eq!(first.status, FrameStatus::Presented);
    assert_eq!(first.quality.samples_completed, None);
    assert_eq!(first.last_completed, None);
    engine.device.complete_submissions();
    engine.width = 640;
    engine.height = 360;
    let mut moved = camera();
    moved.eye.x += 1.0;
    let next = engine.render(&scene, &moved).unwrap();
    let completed = next.last_completed.unwrap();
    assert_eq!(completed.submission_id, first.metrics.last_submission_id);
    assert_eq!(completed.quality.extent, first.quality.extent);
    assert_eq!(
        completed.quality.samples_completed,
        Some(first.quality.samples_submitted)
    );
    assert_eq!(next.quality.extent, [640, 360]);
    assert_eq!(next.quality.samples_completed, None);
    assert!(next.metrics.last_submission_id > completed.submission_id);
}

#[test]
fn a_synced_scene_is_fully_resident_and_an_unsynced_slot_is_not() {
    let mut engine = engine();
    let scene = super::tests::represented_scene(1, 1);
    let image = engine
        .render_image(
            &scene,
            &camera(),
            ImageConfig {
                width: 16,
                height: 16,
            },
        )
        .unwrap();
    assert!(image.quality.full_residency);
    assert_eq!(engine.scene_gpu.pending_drawables(), 0);
    engine.scene_gpu.forget_slot_sync();
    assert!(engine.scene_gpu.pending_drawables() > 0);
    assert!(!engine.effective_quality(1, 1).full_residency);
}

#[test]
fn frame_report_requests_convergence_then_idles_and_restarts_after_camera_motion() {
    let mut engine = engine();
    engine.set_render_mode(RenderMode::Converged);
    let scene = super::tests::represented_scene(1, 1);
    let first = engine.render(&scene, &camera()).expect("first frame");
    assert!(
        first.needs_another_frame,
        "a partial exposure must not idle"
    );
    let mut samples = 1;
    loop {
        engine.device.complete_submissions();
        let next = engine.render(&scene, &camera()).expect("convergence frame");
        samples += 1;
        assert!(samples <= 64, "stable exposure must terminate: {next:?}");
        if !next.needs_another_frame {
            assert_eq!(
                next.quality.samples_submitted,
                next.quality.samples_required
            );
            break;
        }
    }
    engine.device.complete_submissions();
    let mut moved = camera();
    moved.eye.x += 1.0;
    assert!(
        engine
            .render(&scene, &moved)
            .expect("changed view")
            .needs_another_frame
    );
}
