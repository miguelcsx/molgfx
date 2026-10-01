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
