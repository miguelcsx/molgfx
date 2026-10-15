use super::*;
use crate::engine::tests::{camera, engine};
use molgfx_core::Scene;

fn config() -> SequenceConfig {
    SequenceConfig::at_fps(
        ImageConfig {
            width: 32,
            height: 24,
        },
        30,
        2,
    )
    .unwrap_or_else(|error| panic!("sequence config is valid: {error}"))
}

#[test]
fn submission_is_bounded_and_preserves_ticket_order() {
    let mut engine = engine();
    let scene = Scene::new();
    let mut sequence = engine
        .sequence(config())
        .unwrap_or_else(|error| panic!("sequence opens: {error}"));
    let first = sequence
        .submit(&mut engine, &scene, &camera(), 0)
        .unwrap_or_else(|error| panic!("first frame submits: {error}"));
    let second = sequence
        .submit(&mut engine, &scene, &camera(), 1)
        .unwrap_or_else(|error| panic!("second frame submits: {error}"));
    assert!(matches!(
        sequence.submit(&mut engine, &scene, &camera(), 2),
        Err(RenderError::SequenceBackpressure { max_in_flight: 2 })
    ));
    let frames = sequence
        .finish(&mut engine)
        .unwrap_or_else(|error| panic!("sequence drains: {error}"));
    assert_eq!(frames[0].ticket, first);
    assert_eq!(frames[1].ticket, second);
}

#[test]
fn timestamps_are_strictly_increasing() {
    let mut engine = engine();
    let scene = Scene::new();
    let mut sequence = engine
        .sequence(config())
        .unwrap_or_else(|error| panic!("sequence opens: {error}"));
    assert!(sequence.submit(&mut engine, &scene, &camera(), 9).is_ok());
    assert!(matches!(
        sequence.submit(&mut engine, &scene, &camera(), 9),
        Err(RenderError::InvalidSequence { .. })
    ));
}

#[test]
fn invalid_pipeline_depth_is_rejected() {
    let engine = engine();
    let invalid = SequenceConfig {
        max_in_flight: 1,
        ..config()
    };
    assert!(matches!(
        engine.sequence(invalid),
        Err(RenderError::InvalidSequence { .. })
    ));
}

#[test]
fn converged_camera_outputs_complete_independent_exposures() {
    let mut engine = engine();
    let scene = Scene::new();
    let mut sequence = engine.sequence(config()).unwrap();
    let mut first = camera();
    let mut second = camera();
    first.eye.x = 3.0;
    second.eye.x = -3.0;
    sequence.submit(&mut engine, &scene, &first, 0).unwrap();
    sequence.submit(&mut engine, &scene, &second, 1).unwrap();
    let frames = sequence.finish(&mut engine).unwrap();
    for frame in frames {
        let quality = frame.image.quality;
        assert_eq!(quality.samples_required, 64);
        assert_eq!(quality.samples_submitted, 64);
        assert_eq!(quality.samples_completed, Some(64));
        assert!(quality.complete());
        assert!(!quality.adaptive);
    }
}

#[test]
fn progressive_sequence_does_not_certify_a_single_sample_as_complete() {
    let mut engine = engine();
    let scene = Scene::new();
    let mut sequence = engine
        .sequence(config().with_exposure(SequenceExposure::Progressive))
        .unwrap();
    sequence.submit(&mut engine, &scene, &camera(), 0).unwrap();
    let frames = sequence.finish(&mut engine).unwrap();
    let quality = frames[0].image.quality;
    assert_eq!(quality.samples_submitted, 1);
    assert_eq!(quality.samples_completed, Some(1));
    assert!(!quality.complete());
}
