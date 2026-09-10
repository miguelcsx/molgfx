use super::*;

#[test]
fn summaries_report_p50_p95_p99_and_resource_peaks() {
    let samples = (1..=100)
        .map(|value| FrameSample {
            gpu_ns: value,
            cpu_ns: 101 - value,
            frame_ns: value * 2,
            allocations: value % 2,
            upload_bytes: value * 10,
            resident_bytes: value * 100,
            stall_events: value % 3,
        })
        .collect::<Vec<_>>();
    let mut scratch = Vec::new();
    let Ok(summary) = summarize(&samples, &mut scratch) else {
        panic!("samples summarize")
    };
    assert_eq!(summary.gpu_median_ns, 50);
    assert_eq!(summary.gpu_p95_ns, 95);
    assert_eq!(summary.gpu_p99_ns, 99);
    assert_eq!(summary.cpu_median_ns, 50);
    assert_eq!(summary.cpu_p95_ns, 95);
    assert_eq!(summary.cpu_p99_ns, 99);
    assert_eq!(summary.frame_median_ns, 100);
    assert_eq!(summary.frame_p95_ns, 190);
    assert_eq!(summary.frame_p99_ns, 198);
    assert_eq!(summary.max_allocations, 1);
    assert_eq!(summary.max_stall_events, 2);
    assert_eq!(summary.peak_resident_bytes, 10_000);
}

#[test]
fn measured_samples_use_real_counter_deltas_and_reject_resets() {
    let previous = CumulativeTelemetry {
        allocation_events: 4,
        upload_bytes: 256,
        resident_bytes: 64,
        stall_events: 1,
    };
    let current = CumulativeTelemetry {
        allocation_events: 4,
        upload_bytes: 512,
        resident_bytes: 128,
        stall_events: 2,
    };
    let sample = match FrameSample::measured(3, 5, 8, previous, current) {
        Ok(sample) => sample,
        Err(error) => panic!("valid telemetry rejected: {error}"),
    };
    assert_eq!(sample.allocations, 0);
    assert_eq!(sample.upload_bytes, 256);
    assert_eq!(sample.resident_bytes, 128);
    assert_eq!(sample.stall_events, 1);

    assert!(matches!(
        FrameSample::measured(3, 5, 8, current, previous),
        Err(MetricsError::CounterRegression { .. })
    ));
}

#[test]
fn steady_state_summary_reuses_scratch_storage() {
    let samples = [FrameSample::default(); 8];
    let mut scratch = Vec::with_capacity(samples.len());
    let pointer = scratch.as_ptr();
    assert!(summarize(&samples, &mut scratch).is_ok());
    assert_eq!(scratch.as_ptr(), pointer);
}
