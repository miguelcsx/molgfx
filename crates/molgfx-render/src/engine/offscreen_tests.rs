use super::tests::{camera, engine};
use super::*;
use molgfx_core::Scene;

#[test]
fn off_screen_rendering_returns_tightly_packed_rgba_without_a_window_pass() {
    let mut engine = engine();
    let image = match engine.render_image(
        &Scene::new(),
        &camera(),
        ImageConfig {
            width: 3,
            height: 2,
        },
    ) {
        Ok(image) => image,
        Err(error) => panic!("image renders: {error}"),
    };
    assert_eq!((image.width, image.height), (3, 2));
    assert_eq!(image.pixels.len(), 3 * 2 * 4);
}

#[test]
fn zero_sized_off_screen_images_are_typed_errors() {
    let mut engine = engine();
    let Err(error) = engine.render_image(
        &Scene::new(),
        &camera(),
        ImageConfig {
            width: 0,
            height: 2,
        },
    ) else {
        panic!("zero width is invalid")
    };
    assert_eq!(error.code(), "MOLGFX-E0072");
}

#[test]
fn profiling_without_timestamps_still_completes_the_exposure() {
    let mut engine = engine();
    let timing = engine
        .profile_frame(
            &Scene::new(),
            &camera(),
            ImageConfig {
                width: 64,
                height: 64,
            },
            MeasuredOutput::Converged,
        )
        .unwrap();
    assert_eq!(timing.gpu_timing, super::GpuTiming::Unsupported);
    assert_eq!(timing.quality.samples_required, 64);
    assert_eq!(timing.quality.samples_completed, Some(64));
    assert!(timing.quality.complete());
}
