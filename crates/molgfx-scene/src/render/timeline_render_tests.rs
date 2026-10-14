//! Rendering the same timeline twice gives identical frames.

use super::timeline_fixture_tests::morphing_scene;
use super::{FrameSchedule, Renderer};
use sha2::{Digest as _, Sha256};

fn hashes(renderer: &mut Renderer) -> Vec<String> {
    let (mut scene, mut timeline) = morphing_scene();
    let camera = scene.framing_camera(1.0);
    let schedule = FrameSchedule::converged(4.0, 0..5);
    let mut hashes = Vec::new();
    renderer
        .render_timeline(
            &mut scene,
            &mut timeline,
            &camera,
            (64, 64),
            &schedule,
            |_, image| {
                hashes.push(format!("{:x}", Sha256::digest(image.pixels())));
                Ok(())
            },
        )
        .unwrap_or_else(|error| panic!("{error}"));
    hashes
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn rendering_the_same_timeline_twice_gives_identical_frame_hashes() {
    let mut renderer = Renderer::new().unwrap_or_else(|error| panic!("{error}"));
    let first = hashes(&mut renderer);
    let second = hashes(&mut renderer);
    assert_eq!(first.len(), 5);
    assert_eq!(first, second);
    assert_ne!(first[0], first[4], "the morph must change the picture");
}

#[test]
fn an_invalid_schedule_is_refused_before_any_frame_is_rendered() {
    let (mut scene, mut timeline) = morphing_scene();
    let camera = scene.framing_camera(1.0);
    let Ok(mut renderer) = Renderer::new() else {
        return;
    };
    let schedule = FrameSchedule::converged(0.0, 0..3);
    let result = renderer.render_timeline(
        &mut scene,
        &mut timeline,
        &camera,
        (64, 64),
        &schedule,
        |_, _| Err(crate::Error::InvalidSpec("no frame may render".to_owned())),
    );
    assert!(matches!(result, Err(crate::Error::InvalidSpec(m)) if m.contains("frame rate")));
}
