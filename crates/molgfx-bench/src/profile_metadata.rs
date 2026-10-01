//! Serialization outside the engine/allocator measurement boundary.

use molgfx::{FrameTiming, PassTiming};
use serde_json::{Value, json};

/// Detailed completed-output telemetry shared by the frame and corpus harnesses.
#[must_use]
pub fn profile_metadata_json(timing: &FrameTiming, passes: &[PassTiming]) -> Value {
    let cpu = timing.cpu_stages;
    let coverage = timing.pass_coverage;
    json!({
        "gpu_ns": timing.gpu_timing.nanoseconds(),
        "gpu_timing_reason": timing.gpu_timing.unavailable_reason(),
        "gpu_scope": "device-clock envelope of every resolved actual pass; not sum of pass durations",
        "cpu_stages": {
            "preparation_ns": cpu.preparation_ns,
            "recording_ns": cpu.recording_ns,
            "submission_ns": cpu.submission_ns,
            "completion_wait_ns": cpu.completion_wait_ns,
            "timestamp_readback_ns": cpu.timestamp_readback_ns,
        },
        "cpu_stage_scope": "disjoint host elapsed intervals; wait includes scheduling, not CPU utilization",
        "pass_coverage": {
            "passes_seen": coverage.passes_seen,
            "retained_passes": coverage.retained_passes,
            "omitted_passes": coverage.omitted_passes,
            "resolved_passes": coverage.resolved_passes,
        },
        "passes": passes.iter().map(|pass| json!({
            "occurrence": pass.occurrence,
            "label": pass.label,
            "kind": pass.kind_name(),
            "sample": pass.sample,
            "gpu_ns": pass.gpu_timing.nanoseconds(),
            "gpu_timing_reason": pass.gpu_timing.unavailable_reason(),
            "timestamp_ticks": pass.timestamp_ticks,
        })).collect::<Vec<_>>(),
    })
}
