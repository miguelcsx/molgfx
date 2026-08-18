//! Deterministic benchmark measurements and acceptance summaries.

#![forbid(unsafe_code)]

#[cfg(test)]
#[path = "metrics_tests.rs"]
mod tests;

/// One frame's production telemetry.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct FrameSample {
    /// GPU duration in nanoseconds.
    pub gpu_ns: u64,
    /// CPU frame-construction duration in nanoseconds.
    pub cpu_ns: u64,
    /// Host allocations during frame construction.
    pub allocations: u64,
    /// Bytes uploaded this frame.
    pub upload_bytes: u64,
    /// Resident device bytes.
    pub resident_bytes: u64,
    /// Instances retained by GPU culling.
    pub visible_instances: u64,
    /// Resident biological LOD clusters.
    pub lod_clusters: u64,
}

/// Stable summary over a caller-owned sample slice.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FrameSummary {
    /// Median GPU duration.
    pub gpu_median_ns: u64,
    /// Nearest-rank 99th-percentile GPU duration.
    pub gpu_p99_ns: u64,
    /// Median CPU construction duration.
    pub cpu_median_ns: u64,
    /// Maximum allocations observed in a frame.
    pub max_allocations: u64,
    /// Maximum upload bytes observed in a frame.
    pub max_upload_bytes: u64,
    /// Peak resident device bytes.
    pub peak_resident_bytes: u64,
}

/// Summarization failure.
#[derive(Clone, Copy, PartialEq, Eq, Debug, thiserror::Error)]
pub enum MetricsError {
    /// At least one frame sample is required.
    #[error("benchmark summary requires at least one frame")]
    Empty,
}

/// Summarizes samples using caller-owned scratch, allocating nothing when the
/// scratch already holds `samples.len()` values. Sorting is deterministic.
///
/// # Errors
///
/// Returns [`MetricsError::Empty`] when `samples` is empty.
pub fn summarize(
    samples: &[FrameSample],
    scratch: &mut Vec<u64>,
) -> Result<FrameSummary, MetricsError> {
    if samples.is_empty() {
        return Err(MetricsError::Empty);
    }
    scratch.clear();
    scratch.extend(samples.iter().map(|sample| sample.gpu_ns));
    scratch.sort_unstable();
    let gpu_median_ns = percentile(scratch, 50);
    let gpu_p99_ns = percentile(scratch, 99);
    scratch.clear();
    scratch.extend(samples.iter().map(|sample| sample.cpu_ns));
    scratch.sort_unstable();
    Ok(FrameSummary {
        gpu_median_ns,
        gpu_p99_ns,
        cpu_median_ns: percentile(scratch, 50),
        max_allocations: maximum(samples, |sample| sample.allocations),
        max_upload_bytes: maximum(samples, |sample| sample.upload_bytes),
        peak_resident_bytes: maximum(samples, |sample| sample.resident_bytes),
    })
}

fn percentile(sorted: &[u64], percentile: usize) -> u64 {
    let rank = (percentile * sorted.len()).div_ceil(100).saturating_sub(1);
    match sorted.get(rank) {
        Some(value) => *value,
        None => 0,
    }
}

fn maximum(samples: &[FrameSample], value: impl Fn(&FrameSample) -> u64) -> u64 {
    match samples.iter().map(value).max() {
        Some(maximum) => maximum,
        None => 0,
    }
}
