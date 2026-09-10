use super::*;
use crate::engine::tests::{camera, engine};
use pdviewx_core::Scene;

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
