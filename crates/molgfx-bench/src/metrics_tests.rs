use super::*;

#[test]
fn summaries_report_nearest_rank_percentiles_and_distinct_resource_peaks() {
    let samples = (1..=100)
        .map(|value| FrameSample {
            gpu_ns: Some(value),
            cpu_ns: 101 - value,
            frame_ns: value * 2,
            residency_allocation_events: value % 2,
            heap: Some(HeapMeasurement {
                allocations: 7,
                reallocations: 3,
                ..HeapMeasurement::default()
            }),
            upload_bytes: value * 10,
            resident_bytes: value * 100,
            stall_events: value % 3,
        })
        .collect::<Vec<_>>();
    let summary = summarize(&samples, &mut Vec::new()).expect("samples summarize");
    assert_eq!(
        (
            summary.gpu_median_ns,
            summary.gpu_p95_ns,
            summary.gpu_p99_ns
        ),
        (Some(50), Some(95), Some(99))
    );
    assert_eq!(
        (
            summary.cpu_median_ns,
            summary.cpu_p95_ns,
            summary.cpu_p99_ns
        ),
        (50, 95, 99)
    );
    assert_eq!(
        (
            summary.frame_median_ns,
            summary.frame_p95_ns,
            summary.frame_p99_ns
        ),
        (100, 190, 198)
    );
    assert_eq!(summary.max_residency_allocation_events, 1);
    assert_eq!(summary.max_heap_allocations, Some(7));
    assert_eq!(summary.max_heap_reallocations, Some(3));
    assert_eq!(summary.max_stall_events, 2);
    assert_eq!(summary.peak_resident_bytes, 10_000);
}

#[test]
fn unresolved_gpu_samples_are_excluded_without_excluding_real_zero_durations() {
    let samples = [None, Some(0), Some(90), None].map(|gpu_ns| FrameSample {
        gpu_ns,
        frame_ns: 10,
        ..FrameSample::default()
    });
    let summary = summarize(&samples, &mut Vec::new()).expect("summary");
    assert_eq!(summary.gpu_resolved_count, 2);
    assert_eq!(
        (summary.gpu_median_ns, summary.gpu_p95_ns),
        (Some(0), Some(90))
    );
    assert_eq!(summary.output_count, 4);
    assert_eq!(summary.heap_measured_count, 0);
    assert_eq!(summary.max_heap_allocations, None);
    let absent = summarize(&[FrameSample::default()], &mut Vec::new()).expect("summary");
    assert_eq!(
        (
            absent.gpu_resolved_count,
            absent.gpu_median_ns,
            absent.gpu_p95_ns,
            absent.gpu_p99_ns
        ),
        (0, None, None, None)
    );
}

#[test]
fn measured_samples_reject_each_lifetime_counter_regression_but_allow_residency_to_shrink() {
    let previous = CumulativeTelemetry {
        allocation_events: 4,
        upload_bytes: 256,
        resident_bytes: 128,
        stall_events: 1,
    };
    let current = CumulativeTelemetry {
        allocation_events: 4,
        upload_bytes: 512,
        resident_bytes: 64,
        stall_events: 2,
    };
    let sample = FrameSample::measured(Some(3), 5, 8, previous, current).expect("valid telemetry");
    assert_eq!(
        (
            sample.residency_allocation_events,
            sample.upload_bytes,
            sample.resident_bytes,
            sample.stall_events
        ),
        (0, 256, 64, 1)
    );
    assert_eq!(sample.heap, None);
    for (counter, regressed) in [
        (
            "allocation_events",
            CumulativeTelemetry {
                allocation_events: 3,
                ..current
            },
        ),
        (
            "upload_bytes",
            CumulativeTelemetry {
                upload_bytes: 255,
                ..current
            },
        ),
        (
            "stall_events",
            CumulativeTelemetry {
                stall_events: 0,
                ..current
            },
        ),
    ] {
        assert_eq!(
            FrameSample::measured(None, 5, 8, previous, regressed),
            Err(MetricsError::CounterRegression { counter })
        );
    }
}

#[test]
fn allocator_retained_bytes_keep_negative_deltas_and_reallocations_separate() {
    let measured = HeapMeasurement::from(Stats {
        allocations: 2,
        reallocations: 5,
        bytes_allocated: 100,
        bytes_deallocated: 140,
        bytes_reallocated: 20,
        ..Stats::default()
    });
    assert_eq!(measured.retained_bytes, -40);
    assert_eq!((measured.allocations, measured.reallocations), (2, 5));
}

#[test]
fn summaries_reuse_reserved_scratch_and_reject_empty_input() {
    let samples = [FrameSample::default(); 8];
    let mut scratch = Vec::with_capacity(samples.len());
    let pointer = scratch.as_ptr();
    summarize(&samples, &mut scratch).expect("summary");
    assert_eq!(scratch.as_ptr(), pointer);
    assert_eq!(summarize(&[], &mut scratch), Err(MetricsError::Empty));
}

#[test]
fn nearest_rank_handles_small_odd_and_even_samples() {
    for (values, median, p95) in [(vec![8], 8, 8), (vec![9, 1], 1, 9), (vec![3, 1, 2], 2, 3)] {
        let samples = values
            .into_iter()
            .map(|frame_ns| FrameSample {
                frame_ns,
                ..FrameSample::default()
            })
            .collect::<Vec<_>>();
        let summary = summarize(&samples, &mut Vec::new()).expect("summary");
        assert_eq!(
            (summary.frame_median_ns, summary.frame_p95_ns),
            (median, p95)
        );
    }
}
