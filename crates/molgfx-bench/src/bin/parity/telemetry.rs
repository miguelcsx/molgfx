//! Shared latency percentiles without inventing external renderer counters.
use molgfx_bench::gallery::Result;
use molgfx_bench::{FrameSample, summarize};
use serde_json::{Value, json};
use std::io;

#[cfg(test)]
#[path = "telemetry_tests.rs"]
mod tests;

const COUNTERS: [&str; 5] = [
    "residency_allocation_events",
    "upload_bytes",
    "resident_bytes",
    "stall_events",
    "heap",
];
const SUMMARY_COUNTERS: [&str; 6] = [
    "max_residency_allocation_events",
    "max_upload_bytes",
    "peak_resident_bytes",
    "max_stall_events",
    "max_heap_allocations",
    "max_heap_reallocations",
];
const REASON: &str =
    "external adapter does not expose instrumented allocator or renderer residency counters";
const CPU_SCOPE: &str =
    "blocking completed-output API wall time, including GPU completion; not exclusive CPU work";

pub(super) fn normalize_external(report: &mut Value) -> Result<()> {
    let rows = report["samples"]
        .as_array_mut()
        .ok_or_else(|| io::Error::other("engine omitted measured samples"))?;
    let mut samples = Vec::with_capacity(rows.len());
    for row in rows {
        let sample = &row["sample"];
        let cpu_ns = sample["cpu_ns"]
            .as_u64()
            .ok_or_else(|| io::Error::other("missing cpu_ns"))?;
        let frame_ns = sample["frame_ns"]
            .as_u64()
            .ok_or_else(|| io::Error::other("missing completed frame_ns"))?;
        // Only latency members enter the shared summarizer; its renderer-counter
        // output is removed below because those counters were not observed.
        samples.push(FrameSample {
            gpu_ns: sample["gpu_ns"].as_u64(),
            cpu_ns,
            frame_ns,
            ..FrameSample::default()
        });
        unavailable(row);
    }
    let mut scratch = Vec::with_capacity(samples.len());
    let mut summary = serde_json::to_value(summarize(&samples, &mut scratch)?)?;
    for field in SUMMARY_COUNTERS {
        summary[field] = Value::Null;
        summary[format!("{field}_unavailable_reason")] = json!(REASON);
    }
    summary["cpu_timing_scope"] = json!(CPU_SCOPE);
    summary["heap_measured_count"] = json!(0);
    summary["heap_unavailable_reason"] = json!(REASON);
    summary["gpu_unavailable_reasons"] = json!(report["samples"].as_array().map(|rows| {
        rows.iter()
            .filter_map(|r| r["gpu_unavailable_reason"].as_str())
            .collect::<Vec<_>>()
    }));
    report["summary"] = summary;
    report["cpu_timing_scope"] = json!(CPU_SCOPE);
    if let Some(row) = report.get_mut("cold_output").filter(|row| !row.is_null()) {
        unavailable(row);
    }
    if let Some(rows) = report["warmup_samples"].as_array_mut() {
        for row in rows {
            unavailable(row);
        }
    }
    for field in ["cold_output", "warmup_samples"] {
        if report.get(field).is_none() {
            report[field] = Value::Null;
            report[format!("{field}_unavailable_reason")] =
                json!("adapter did not record this phase; measured outputs cannot reconstruct it");
        }
    }
    Ok(())
}

fn unavailable(row: &mut Value) {
    for field in COUNTERS {
        row["sample"][field] = Value::Null;
        row[format!("{field}_unavailable_reason")] = json!(REASON);
    }
    for field in ["cpu_stages", "pass_coverage", "passes"] {
        row[field] = Value::Null;
        row[format!("{field}_unavailable_reason")] =
            json!("external adapter does not expose production stage or per-pass instrumentation");
    }
    row["cpu_timing_scope"] = json!(CPU_SCOPE);
    if row["sample"]["gpu_ns"].is_null() && row.get("gpu_unavailable_reason").is_none() {
        row["gpu_unavailable_reason"] =
            json!("external adapter does not expose GPU timestamp queries");
    }
}
