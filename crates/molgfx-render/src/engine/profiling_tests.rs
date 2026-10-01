use super::*;
use crate::testing::MockDevice;
use std::future::Future as _;
use std::sync::atomic::Ordering;

#[test]
fn cancelling_a_submitted_profile_preserves_storage_and_waits_before_reuse() {
    for held_fence in [1, 2] {
        let mut engine = super::super::tests::engine();
        engine.device = MockDevice::with_timestamp_queries();
        engine.queue = engine.device.queue();
        engine.profiler =
            GpuProfiler::new(&engine.device, molgfx_gpu::TextureFormat::Bgra8Unorm).unwrap();
        let scene = Scene::new();
        let camera = super::super::tests::camera();
        let config = ImageConfig {
            width: 8,
            height: 8,
        };
        let log = std::sync::Arc::clone(&engine.device.log);
        log.hold_fence_from.store(held_fence, Ordering::Release);
        log.hold_fence.store(true, Ordering::Release);
        let mut context = std::task::Context::from_waker(std::task::Waker::noop());
        let mut future = Box::pin(engine.profile_frame_async(&scene, &camera, config));
        assert!(future.as_mut().poll(&mut context).is_pending());
        drop(future);
        assert_eq!(log.submitted_fence.load(Ordering::Acquire), held_fence);
        let writes = log.writes.lock().unwrap().len();
        let mut resumed = Box::pin(engine.profile_frame_async(&scene, &camera, config));
        assert!(resumed.as_mut().poll(&mut context).is_pending());
        assert_eq!(log.submitted_fence.load(Ordering::Acquire), held_fence);
        assert_eq!(log.writes.lock().unwrap().len(), writes);
        log.hold_fence.store(false, Ordering::Release);
        let timing = pollster::block_on(resumed).expect("profile resumes after cancellation");
        assert!(timing.quality.complete());
        assert_eq!(log.submitted_fence.load(Ordering::Acquire), held_fence + 2);
        assert_eq!(log.completed_fence.load(Ordering::Acquire), held_fence + 2);
    }
}

#[test]
fn sample_preparation_failure_does_not_destroy_reusable_capture() {
    let mut engine = super::super::tests::engine();
    let scene = Scene::new();
    let camera = super::super::tests::camera();
    let config = ImageConfig {
        width: 8,
        height: 8,
    };
    let (started, mut exposure) = engine.prepare_profile(&scene, &camera, config).unwrap();
    exposure.samples = 65;
    let mut profiler = engine.profiler.take().unwrap();
    let error = engine
        .submit_profile(&mut profiler, started, config, exposure)
        .unwrap_err();
    assert!(matches!(
        error,
        RenderError::Residency {
            reason: "exposure exceeds its uniform arena budget"
        }
    ));
    engine.profiler = Some(profiler);
    let timing = engine
        .profile_frame(&scene, &camera, config)
        .expect("profile resumes after a recording failure");
    assert!(timing.quality.complete());
}

#[test]
fn profiling_target_uses_the_render_pipeline_format() {
    let device = MockDevice::with_timestamp_queries();
    let mut profiler = GpuProfiler::new(&device, molgfx_gpu::TextureFormat::Bgra8Unorm)
        .unwrap_or_else(|error| panic!("profiler opens: {error}"))
        .unwrap_or_else(|| panic!("timestamp-capable mock creates a profiler"));
    profiler
        .ensure_target(
            &device,
            ImageConfig {
                width: 1920,
                height: 1080,
            },
        )
        .unwrap_or_else(|error| panic!("target allocates: {error}"));
    let formats = device
        .log
        .texture_formats
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert!(formats.contains(&("profiling target", molgfx_gpu::TextureFormat::Bgra8Unorm)));
}

#[test]
fn timestamp_pairs_do_not_cross_output_boundaries() {
    for (start, end, expected) in [
        (1_000_u64, 1_750_u64, super::super::GpuTiming::Resolved(750)),
        (2_000, 1_750, super::super::GpuTiming::InvalidOrder),
        (0, 0, super::super::GpuTiming::Unresolved),
        (4_000, 4_000, super::super::GpuTiming::Resolved(0)),
    ] {
        let mut bytes = [0; 16];
        bytes[..8].copy_from_slice(&start.to_le_bytes());
        bytes[8..].copy_from_slice(&end.to_le_bytes());
        assert_eq!(
            GpuProfiler::<MockDevice>::decode_timing(&bytes, 1.0).unwrap(),
            expected
        );
    }
}

#[test]
fn long_gpu_intervals_are_not_clamped_to_u32_ticks() {
    let mut bytes = [0; 16];
    bytes[..8].copy_from_slice(&1_u64.to_le_bytes());
    bytes[8..].copy_from_slice(&5_000_000_001_u64.to_le_bytes());
    let timing = GpuProfiler::<MockDevice>::decode_timing(&bytes, 1.0).unwrap();
    assert_eq!(timing.nanoseconds(), Some(5_000_000_000));
}

#[test]
fn invalid_timestamp_payloads_never_become_zero_durations() {
    use super::super::GpuTiming;
    assert_eq!(
        GpuProfiler::<MockDevice>::decode_timing(&[0; 8], 1.0).unwrap(),
        GpuTiming::Malformed
    );
    let mut bytes = [0; 16];
    bytes[..8].copy_from_slice(&1_u64.to_le_bytes());
    bytes[8..].copy_from_slice(&2_u64.to_le_bytes());
    for period in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        let timing = GpuProfiler::<MockDevice>::decode_timing(&bytes, period).unwrap();
        assert_eq!(timing, GpuTiming::InvalidPeriod);
        assert_eq!(timing.nanoseconds(), None);
        assert_eq!(timing.unavailable_reason(), Some("invalid_period"));
    }
}
