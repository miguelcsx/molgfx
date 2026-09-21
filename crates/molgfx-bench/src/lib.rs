//! Deterministic benchmark measurements and acceptance summaries.

#![forbid(unsafe_code)]

pub mod fixtures;
pub mod reader;

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
    /// Conservative end-to-end blocking frame duration in nanoseconds.
    pub frame_ns: u64,
    /// Host allocations during frame construction.
    pub allocations: u64,
    /// Bytes uploaded this frame.
    pub upload_bytes: u64,
    /// Resident device bytes.
    pub resident_bytes: u64,
    /// Capacity or budget stalls during this frame.
    pub stall_events: u64,
}

/// Cumulative counters sampled from the production renderer.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct CumulativeTelemetry {
    /// Residency-owned host allocation events since engine creation.
    pub allocation_events: u64,
    /// Bytes submitted through the upload ring since engine creation.
    pub upload_bytes: u64,
    /// Current resident device bytes.
    pub resident_bytes: u64,
    /// Capacity and budget stalls since engine creation.
    pub stall_events: u64,
}

impl FrameSample {
    /// Builds a frame from two real cumulative renderer snapshots.
    ///
    /// # Errors
    ///
    /// Returns a typed error if a cumulative counter moved backwards.
    pub fn measured(
        gpu_ns: u64,
        cpu_ns: u64,
        frame_ns: u64,
        previous: CumulativeTelemetry,
        current: CumulativeTelemetry,
    ) -> Result<Self, MetricsError> {
        Ok(Self {
            gpu_ns,
            cpu_ns,
            frame_ns,
            allocations: delta(
                "allocation_events",
                previous.allocation_events,
                current.allocation_events,
            )?,
            upload_bytes: delta("upload_bytes", previous.upload_bytes, current.upload_bytes)?,
            resident_bytes: current.resident_bytes,
            stall_events: delta("stall_events", previous.stall_events, current.stall_events)?,
        })
    }
}

/// Stable summary over a caller-owned sample slice.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FrameSummary {
    /// Median GPU duration.
    pub gpu_median_ns: u64,
    /// Nearest-rank 95th-percentile GPU duration.
    pub gpu_p95_ns: u64,
    /// Nearest-rank 99th-percentile GPU duration.
    pub gpu_p99_ns: u64,
    /// Median CPU construction duration.
    pub cpu_median_ns: u64,
    /// Nearest-rank 95th-percentile CPU construction duration.
    pub cpu_p95_ns: u64,
    /// Nearest-rank 99th-percentile CPU construction duration.
    pub cpu_p99_ns: u64,
    /// Median end-to-end blocking frame duration.
    pub frame_median_ns: u64,
    /// Nearest-rank 95th-percentile end-to-end frame duration.
    pub frame_p95_ns: u64,
    /// Nearest-rank 99th-percentile end-to-end frame duration.
    pub frame_p99_ns: u64,
    /// Maximum allocations observed in a frame.
    pub max_allocations: u64,
    /// Maximum upload bytes observed in a frame.
    pub max_upload_bytes: u64,
    /// Peak resident device bytes.
    pub peak_resident_bytes: u64,
    /// Maximum capacity or budget stalls observed in one frame.
    pub max_stall_events: u64,
}

/// Summarization failure.
#[derive(Clone, Copy, PartialEq, Eq, Debug, thiserror::Error)]
pub enum MetricsError {
    /// At least one frame sample is required.
    #[error("benchmark summary requires at least one frame")]
    Empty,
    /// A renderer lifetime counter moved backwards between samples.
    #[error("cumulative telemetry counter {counter} moved backwards")]
    CounterRegression {
        /// Stable counter name.
        counter: &'static str,
    },
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
    let gpu_p95_ns = percentile(scratch, 95);
    let gpu_p99_ns = percentile(scratch, 99);
    scratch.clear();
    scratch.extend(samples.iter().map(|sample| sample.cpu_ns));
    scratch.sort_unstable();
    let cpu_median_ns = percentile(scratch, 50);
    let cpu_p95_ns = percentile(scratch, 95);
    let cpu_p99_ns = percentile(scratch, 99);
    scratch.clear();
    scratch.extend(samples.iter().map(|sample| sample.frame_ns));
    scratch.sort_unstable();
    Ok(FrameSummary {
        gpu_median_ns,
        gpu_p95_ns,
        gpu_p99_ns,
        cpu_median_ns,
        cpu_p95_ns,
        cpu_p99_ns,
        frame_median_ns: percentile(scratch, 50),
        frame_p95_ns: percentile(scratch, 95),
        frame_p99_ns: percentile(scratch, 99),
        max_allocations: maximum(samples, |sample| sample.allocations),
        max_upload_bytes: maximum(samples, |sample| sample.upload_bytes),
        peak_resident_bytes: maximum(samples, |sample| sample.resident_bytes),
        max_stall_events: maximum(samples, |sample| sample.stall_events),
    })
}

fn delta(counter: &'static str, previous: u64, current: u64) -> Result<u64, MetricsError> {
    current
        .checked_sub(previous)
        .ok_or(MetricsError::CounterRegression { counter })
}

fn percentile(sorted: &[u64], percentile: usize) -> u64 {
    let rank = (percentile * sorted.len()).div_ceil(100).saturating_sub(1);
    match sorted.get(rank) {
        Some(value) => *value,
        None => 0,
    }
}

fn maximum(samples: &[FrameSample], value: impl Fn(&FrameSample) -> u64) -> u64 {
    samples
        .iter()
        .map(value)
        .max()
        .into_iter()
        .fold(0, |_, maximum| maximum)
}
