use super::tests::{camera, engine};
use super::{Engine, EngineConfig};
use crate::RenderError;
use crate::testing::MockDevice;
use pdviewx_core::Scene;

#[test]
fn steady_frames_reuse_residency_storage_and_report_real_bytes() {
    let mut engine = engine();
    let scene = Scene::new();
    let initial = engine.residency_metrics();
    for _ in 0..2 {
        if let Err(error) = engine.render(&scene, &camera()) {
            panic!("steady frame renders: {error}")
        }
    }
    let metrics = engine.residency_metrics();
    let uniform_bytes = std::mem::size_of::<crate::scene_gpu::FrameUniforms>() as u64;
    assert_eq!(metrics.uploads.bytes_staged, uniform_bytes * 2);
    assert_eq!(metrics.uploads.bytes_submitted, uniform_bytes * 2);
    assert_eq!(metrics.uploads.bytes_retired, uniform_bytes * 2);
    assert_eq!(metrics.uploads.stall_events, 0);
    assert!(metrics.arena.resident_bytes >= uniform_bytes);
    assert_eq!(metrics.machine.resident_resources, 1);
    assert_eq!(
        metrics.uploads.host_allocation_events,
        initial.uploads.host_allocation_events
    );
    assert_eq!(
        metrics.commands.host_allocation_events,
        initial.commands.host_allocation_events
    );
}

#[test]
fn frame_upload_budget_exhaustion_is_typed_and_profiled() {
    let mut config = EngineConfig::default();
    config.residency.uploads.epoch_budget_bytes = 1;
    let mut engine = match Engine::<MockDevice>::new(&config, None) {
        Ok(engine) => engine,
        Err(error) => panic!("small epoch budget is a runtime policy: {error}"),
    };
    let error = engine.render(&Scene::new(), &camera());
    assert!(matches!(error, Err(RenderError::Residency { .. })));
    let metrics = engine.residency_metrics();
    assert_eq!(metrics.uploads.stall_events, 1);
    assert_eq!(
        metrics.uploads.stalled_bytes,
        std::mem::size_of::<crate::scene_gpu::FrameUniforms>() as u64
    );
}
