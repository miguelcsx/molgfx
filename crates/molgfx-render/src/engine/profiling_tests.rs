use super::*;
use crate::testing::MockDevice;

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
fn same_submission_timestamps_use_the_current_start() {
    assert_eq!(timestamp_delta(1_000, 1_750, Some(100)), Some(750));
}

#[test]
fn delayed_pass_end_timestamps_pair_with_the_previous_start() {
    assert_eq!(timestamp_delta(2_000, 1_750, Some(1_000)), Some(750));
}

#[test]
fn the_first_delayed_sample_is_unresolved() {
    assert_eq!(timestamp_delta(2_000, 1_750, None), None);
}

#[test]
fn an_all_zero_query_readback_is_unresolved() {
    assert_eq!(timestamp_delta(0, 0, None), None);
}

#[test]
fn equal_nonzero_timestamps_are_a_resolved_zero() {
    assert_eq!(timestamp_delta(4_000, 4_000, None), Some(0));
}
