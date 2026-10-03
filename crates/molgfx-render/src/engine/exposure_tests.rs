use super::ImageConfig;
use super::exposure::EXPOSURE_RUN;
use super::tests::{camera, engine};
use molgfx_core::Scene;

#[test]
fn a_converged_image_is_submitted_in_bounded_runs_covering_every_sample() {
    let mut engine = engine();
    let image = match engine.render_image(
        &Scene::new(),
        &camera(),
        ImageConfig {
            width: 4,
            height: 4,
        },
    ) {
        Ok(image) => image,
        Err(error) => panic!("image renders: {error}"),
    };
    let samples = image.quality.samples_required;
    assert!(samples > EXPOSURE_RUN, "the test must span several runs");
    let Ok(submits) = engine.device.log.submits.lock() else {
        panic!("log lock")
    };
    assert_eq!(
        *submits,
        samples.div_ceil(EXPOSURE_RUN),
        "one submission per run of samples"
    );
    drop(submits);
    let uniforms = engine.scene_gpu.residency_metrics().exposure_upload_bytes;
    let stride = uniforms / u64::from(samples);
    assert_eq!(
        uniforms,
        stride * u64::from(samples),
        "every sample's uniforms are uploaded exactly once"
    );
}

#[test]
fn transparent_passes_read_a_depth_copy_only_where_the_live_depth_cannot_be_bound() {
    use crate::engine::{Engine, EngineConfig};
    use crate::testing::MockDevice;
    let copies = |device: MockDevice| {
        let mut engine = Engine::<MockDevice>::from_opened(
            &EngineConfig::default(),
            MockDevice::opened_with(device),
        )
        .unwrap_or_else(|error| panic!("mock engine opens: {error}"));
        if let Err(error) = engine.render(&Scene::new(), &camera()) {
            panic!("frame renders: {error}")
        }
        let Ok(copies) = engine.device.log.texture_copies.lock() else {
            panic!("log lock")
        };
        copies.clone()
    };
    assert!(copies(MockDevice::default()).is_empty());
    assert_eq!(
        copies(MockDevice::without_depth_read_while_sampled()),
        vec![("frame depth", "opaque depth snapshot")]
    );
}
