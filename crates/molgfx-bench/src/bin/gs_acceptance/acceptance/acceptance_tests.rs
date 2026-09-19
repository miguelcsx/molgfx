use super::cli::validate_sampling;
use super::{Availability, GoldenSceneId, MetricSet, golden_scene_specs};
use molgfx_bench::FrameSummary;

#[test]
fn ladder_declares_every_normative_scene_once() {
    let scenes = golden_scene_specs();
    let ids = scenes.map(|scene| scene.id.as_str());
    assert_eq!(
        ids,
        [
            "GS-001", "GS-002", "GS-003", "GS-004", "GS-005", "GS-006", "GS-007", "GS-008",
            "GS-009", "GS-010",
        ]
    );
}

#[test]
fn provider_fixtures_are_explicitly_absent() {
    let scenes = golden_scene_specs();
    let gs006 = scenes.iter().find(|scene| scene.id == GoldenSceneId::Gs006);
    let gs008 = scenes.iter().find(|scene| scene.id == GoldenSceneId::Gs008);
    assert!(gs006.is_some_and(|scene| scene.default_fixture.is_none()));
    assert!(gs008.is_some_and(|scene| scene.default_fixture.is_none()));
}

#[test]
fn acceptance_sampling_requires_warmup_and_enough_percentile_frames() {
    assert!(validate_sampling(20, 120).is_ok());
    assert!(validate_sampling(0, 120).is_err());
    assert!(validate_sampling(20, 99).is_err());
}

#[test]
fn unavailable_metric_serializes_a_reason_without_a_numeric_sentinel() {
    let metric = Availability::<u64>::Unavailable {
        reason: "backend has no VRAM counter".into(),
    };
    let encoded = serde_json::to_value(metric).expect("test JSON serialization must succeed");
    assert_eq!(encoded["status"], "unavailable");
    assert_eq!(encoded["reason"], "backend has no VRAM counter");
    assert!(encoded.get("value").is_none());
}

#[test]
fn unresolved_gpu_timestamps_never_serialize_zero_as_available() {
    let metrics = MetricSet::measured(120, summary_with_gpu_zero(), false, 0);
    let encoded = serde_json::to_value(metrics).expect("test metrics serialize");
    for name in ["gpu_p50_ns", "gpu_p95_ns", "gpu_p99_ns"] {
        assert_eq!(encoded[name]["status"], "unavailable");
        assert!(encoded[name].get("value").is_none());
    }
    assert_eq!(encoded["cpu_p50_ns"]["status"], "available");
    assert_eq!(encoded["frame_p99_ns"]["status"], "available");
}

#[test]
fn a_resolved_gpu_zero_remains_explicitly_available() {
    let metrics = MetricSet::measured(120, summary_with_gpu_zero(), true, 0);
    let encoded = serde_json::to_value(metrics).expect("test metrics serialize");
    assert_eq!(encoded["gpu_p50_ns"]["status"], "available");
    assert_eq!(encoded["gpu_p50_ns"]["value"], 0);
    assert_eq!(
        encoded["max_process_heap_allocations"]["status"],
        "available"
    );
    assert_eq!(encoded["max_process_heap_allocations"]["value"], 0);
}

fn summary_with_gpu_zero() -> FrameSummary {
    FrameSummary {
        gpu_median_ns: 0,
        gpu_p95_ns: 0,
        gpu_p99_ns: 0,
        cpu_median_ns: 10,
        cpu_p95_ns: 20,
        cpu_p99_ns: 30,
        frame_median_ns: 40,
        frame_p95_ns: 50,
        frame_p99_ns: 60,
        max_allocations: 0,
        max_upload_bytes: 0,
        peak_resident_bytes: 0,
        max_stall_events: 0,
    }
}
