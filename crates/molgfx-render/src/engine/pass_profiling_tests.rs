use super::*;
use crate::testing::MockDevice;

#[test]
fn old_same_slot_ticks_do_not_make_a_new_output_appear_resolved() {
    let (mut profiler, bytes) = profile(&[(100, 200), (300, 400)], 2);
    assert_eq!(
        profiler.decode(&bytes, 1.0).unwrap(),
        GpuTiming::Resolved(300)
    );
    profiler.decoded.clear();
    let (_, bytes) = profile(&[(100, 200), (500, 600)], 2);
    assert_eq!(profiler.decode(&bytes, 1.0).unwrap(), GpuTiming::Stale);
    assert_eq!(profiler.timings()[0].timestamp_ticks, Some([100, 200]));
    assert_eq!(profiler.timings()[0].gpu_timing, GpuTiming::Stale);
    assert_eq!(profiler.coverage().resolved_passes, 1);
    profiler.decoded.clear();
    let (_, bytes) = profile(&[(700, 750), (710, 730)], 2);
    assert_eq!(
        profiler.decode(&bytes, 1.0).unwrap(),
        GpuTiming::Resolved(50)
    );
    assert_eq!(profiler.coverage().resolved_passes, 2);
}

#[test]
fn coarse_equal_ticks_at_the_previous_boundary_are_not_stale() {
    let (mut profiler, bytes) = profile(&[(400, 400)], 1);
    assert_eq!(
        profiler.decode(&bytes, 1.0).unwrap(),
        GpuTiming::Resolved(0)
    );
    profiler.decoded.clear();
    assert_eq!(
        profiler.decode(&bytes, 1.0).unwrap(),
        GpuTiming::Resolved(0)
    );
}

fn profile(pairs: &[(u64, u64)], capacity: u32) -> (PassProfiler<MockDevice>, Vec<u8>) {
    let device = MockDevice::with_timestamp_queries();
    let mut profiler = PassProfiler::new(&device).unwrap();
    let mut capture = PassTimestampCapture::new(&device, capacity).unwrap();
    let mut bytes = Vec::new();
    for &(start, end) in pairs {
        if capture
            .record_pass("parallel", TimestampPassKind::Compute, false)
            .is_some()
        {
            bytes.extend_from_slice(&start.to_le_bytes());
            bytes.extend_from_slice(&end.to_le_bytes());
        }
    }
    profiler.capture = Some(capture);
    (profiler, bytes)
}

#[test]
fn gpu_envelope_covers_overlapping_passes_not_just_the_last_record() {
    let (mut profiler, bytes) = profile(&[(100, 400), (150, 250)], 2);
    assert_eq!(
        profiler.decode(&bytes, 1.0).unwrap(),
        GpuTiming::Resolved(300)
    );
    assert_eq!(profiler.timings()[0].gpu_timing, GpuTiming::Resolved(300));
    assert_eq!(profiler.timings()[1].gpu_timing, GpuTiming::Resolved(100));
    assert_eq!(profiler.coverage().resolved_passes, 2);
}

#[test]
fn unavailable_and_overflowed_passes_cannot_certify_the_whole_interval() {
    for (bad, reason) in [
        ((0, 250), GpuTiming::Unresolved),
        ((300, 250), GpuTiming::InvalidOrder),
    ] {
        let (mut profiler, bytes) = profile(&[(100, 400), bad], 2);
        assert_eq!(profiler.decode(&bytes, 1.0).unwrap(), reason);
        assert_eq!(profiler.timings()[1].timestamp_ticks, Some([bad.0, bad.1]));
        assert_eq!(profiler.coverage().resolved_passes, 1);
    }
    let (mut profiler, bytes) = profile(&[(100, 400), (150, 250), (160, 260)], 2);
    assert_eq!(
        profiler.decode(&bytes, 1.0).unwrap(),
        GpuTiming::CapacityExceeded
    );
    assert_eq!(profiler.coverage().passes_seen, 3);
    assert_eq!(profiler.coverage().retained_passes, 2);
    assert_eq!(profiler.coverage().omitted_passes, 1);
}
