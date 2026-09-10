use crate::{CoreError, Scene};

#[test]
fn presentation_time_changes_only_for_a_distinct_finite_value() {
    let mut scene = Scene::new();
    scene
        .set_presentation_time(0.25)
        .unwrap_or_else(|error| panic!("time should be valid: {error}"));
    let revision = scene.presentation_revision();
    scene
        .set_presentation_time(0.25)
        .unwrap_or_else(|error| panic!("same time should remain valid: {error}"));
    assert_eq!(
        scene.presentation_time_seconds().to_bits(),
        0.25f32.to_bits()
    );
    assert_eq!(scene.presentation_revision(), revision);
}

#[test]
fn presentation_time_rejects_values_outside_the_gpu_clock() {
    let mut scene = Scene::new();
    assert!(matches!(
        scene.set_presentation_time(f64::MAX),
        Err(CoreError::InvalidTimeline {
            reason: "presentation time must be a finite portable f32 value",
        })
    ));
}
