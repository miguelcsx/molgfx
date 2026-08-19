use super::*;

#[test]
fn summaries_report_median_p99_and_resource_peaks() {
    let samples = (1..=100)
        .map(|value| FrameSample {
            gpu_ns: value,
            cpu_ns: 101 - value,
            frame_ns: value * 2,
            allocations: value % 2,
            upload_bytes: value * 10,
            resident_bytes: value * 100,
            visible_instances: value,
            lod_clusters: 1,
        })
        .collect::<Vec<_>>();
    let mut scratch = Vec::new();
    let Ok(summary) = summarize(&samples, &mut scratch) else {
        panic!("samples summarize")
    };
    assert_eq!(summary.gpu_median_ns, 50);
    assert_eq!(summary.gpu_p99_ns, 99);
    assert_eq!(summary.cpu_median_ns, 50);
    assert_eq!(summary.frame_median_ns, 100);
    assert_eq!(summary.frame_p99_ns, 198);
    assert_eq!(summary.max_allocations, 1);
    assert_eq!(summary.peak_resident_bytes, 10_000);
}

#[test]
fn steady_state_summary_reuses_scratch_storage() {
    let samples = [FrameSample::default(); 8];
    let mut scratch = Vec::with_capacity(samples.len());
    let pointer = scratch.as_ptr();
    assert!(summarize(&samples, &mut scratch).is_ok());
    assert_eq!(scratch.as_ptr(), pointer);
}
