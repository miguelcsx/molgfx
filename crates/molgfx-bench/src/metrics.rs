//! Completed-output timing, renderer counter deltas, and allocator measurements.

use serde::Serialize;
use stats_alloc::{INSTRUMENTED_SYSTEM, Region, Stats};

#[cfg(test)]
#[path = "metrics_tests.rs"]
mod tests;

/// Allocator activity in a measured region, not RSS or device residency.
/// The consumer binary must install `stats_alloc::INSTRUMENTED_SYSTEM`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize)]
pub struct HeapMeasurement {
    /// Allocation calls, including backend Rust allocations in this process.
    pub allocations: usize,
    /// Reallocation calls, reported independently of allocation calls.
    pub reallocations: usize,
    /// Bytes requested, including growth during reallocations.
    pub allocated_bytes: usize,
    /// Bytes released, including shrinking reallocations.
    pub deallocated_bytes: usize,
    /// Net retained heap bytes; negative when preexisting storage was freed.
    pub retained_bytes: i128,
}

impl From<Stats> for HeapMeasurement {
    fn from(stats: Stats) -> Self {
        Self {
            allocations: stats.allocations,
            reallocations: stats.reallocations,
            allocated_bytes: stats.bytes_allocated,
            deallocated_bytes: stats.bytes_deallocated,
            retained_bytes: stats.bytes_allocated as i128 - stats.bytes_deallocated as i128,
        }
    }
}

/// Measures a consumer operation with the process-wide instrumented allocator.
/// Fixtures, output storage and sorting scratch should be reserved beforehand.
/// Native driver allocations are outside this allocator's scope.
pub fn measure_heap<T>(operation: impl FnOnce() -> T) -> (HeapMeasurement, T) {
    let region = Region::new(&INSTRUMENTED_SYSTEM);
    let value = operation();
    (region.change().into(), value)
}

/// One completed output's production telemetry.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize)]
pub struct FrameSample {
    /// Resolved GPU duration; absent timestamps are never represented as zero.
    pub gpu_ns: Option<u64>,
    /// CPU construction duration in nanoseconds.
    pub cpu_ns: u64,
    /// End-to-end blocking duration, including completion, in nanoseconds.
    pub frame_ns: u64,
    /// Residency-owned allocation events, not all process heap allocations.
    pub residency_allocation_events: u64,
    /// Separately instrumented heap activity, absent when not measured.
    pub heap: Option<HeapMeasurement>,
    /// Bytes uploaded during this output.
    pub upload_bytes: u64,
    /// Logical resident device bytes, not physical VRAM or process RSS.
    pub resident_bytes: u64,
    /// Capacity or budget stalls during this output.
    pub stall_events: u64,
}

/// Cumulative counters sampled from the production renderer.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct CumulativeTelemetry {
    /// Residency-owned allocation events since engine creation.
    pub allocation_events: u64,
    /// Bytes submitted through the upload ring since engine creation.
    pub upload_bytes: u64,
    /// Current logical resident device bytes.
    pub resident_bytes: u64,
    /// Capacity and budget stalls since engine creation.
    pub stall_events: u64,
}

impl FrameSample {
    /// Builds a sample from real cumulative renderer snapshots.
    /// Heap counters must be attached from a separately measured allocator region.
    ///
    /// # Errors
    /// Returns a typed error if a cumulative counter moved backwards.
    pub fn measured(
        gpu_ns: Option<u64>,
        cpu_ns: u64,
        frame_ns: u64,
        previous: CumulativeTelemetry,
        current: CumulativeTelemetry,
    ) -> Result<Self, MetricsError> {
        Ok(Self {
            gpu_ns,
            cpu_ns,
            frame_ns,
            residency_allocation_events: delta(
                "allocation_events",
                previous.allocation_events,
                current.allocation_events,
            )?,
            heap: None,
            upload_bytes: delta("upload_bytes", previous.upload_bytes, current.upload_bytes)?,
            resident_bytes: current.resident_bytes,
            stall_events: delta("stall_events", previous.stall_events, current.stall_events)?,
        })
    }
}

/// Stable nearest-rank summary over a caller-owned sample slice.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize)]
pub struct FrameSummary {
    /// Outputs contributing to latency and CPU percentiles.
    pub output_count: usize,
    /// Outputs whose GPU timestamps resolved successfully.
    pub gpu_resolved_count: usize,
    /// Median resolved GPU duration, absent when no timestamps resolved.
    pub gpu_median_ns: Option<u64>,
    /// Nearest-rank 95th-percentile resolved GPU duration.
    pub gpu_p95_ns: Option<u64>,
    /// Nearest-rank 99th-percentile resolved GPU duration.
    pub gpu_p99_ns: Option<u64>,
    /// Median CPU construction duration.
    pub cpu_median_ns: u64,
    /// Nearest-rank 95th-percentile CPU construction duration.
    pub cpu_p95_ns: u64,
    /// Nearest-rank 99th-percentile CPU construction duration.
    pub cpu_p99_ns: u64,
    /// Median end-to-end blocking output duration.
    pub frame_median_ns: u64,
    /// Nearest-rank 95th-percentile end-to-end output duration.
    pub frame_p95_ns: u64,
    /// Nearest-rank 99th-percentile end-to-end output duration.
    pub frame_p99_ns: u64,
    /// Maximum residency-owned allocation events in one output.
    pub max_residency_allocation_events: u64,
    /// Outputs with allocator measurements.
    pub heap_measured_count: usize,
    /// Maximum measured heap allocation calls in one output.
    pub max_heap_allocations: Option<usize>,
    /// Maximum measured heap reallocation calls in one output.
    pub max_heap_reallocations: Option<usize>,
    /// Maximum upload bytes in one output.
    pub max_upload_bytes: u64,
    /// Peak logical resident device bytes.
    pub peak_resident_bytes: u64,
    /// Maximum capacity or budget stalls in one output.
    pub max_stall_events: u64,
}

/// Summarization or renderer-counter failure.
#[derive(Clone, Copy, PartialEq, Eq, Debug, thiserror::Error)]
pub enum MetricsError {
    /// At least one output sample is required.
    #[error("benchmark summary requires at least one output")]
    Empty,
    /// A renderer lifetime counter moved backwards between snapshots.
    #[error("cumulative telemetry counter {counter} moved backwards")]
    CounterRegression {
        /// Stable counter name.
        counter: &'static str,
    },
}

/// Summarizes using reserved caller-owned scratch without per-output allocation.
/// Sorting is deterministic; unresolved GPU samples do not enter percentiles.
///
/// # Errors
/// Returns [`MetricsError::Empty`] when `samples` is empty.
pub fn summarize(
    samples: &[FrameSample],
    scratch: &mut Vec<u64>,
) -> Result<FrameSummary, MetricsError> {
    if samples.is_empty() {
        return Err(MetricsError::Empty);
    }
    scratch.clear();
    scratch.extend(samples.iter().filter_map(|sample| sample.gpu_ns));
    scratch.sort_unstable();
    let gpu_resolved_count = scratch.len();
    let gpu_median_ns = percentile(scratch, 50);
    let gpu_p95_ns = percentile(scratch, 95);
    let gpu_p99_ns = percentile(scratch, 99);
    scratch.clear();
    scratch.extend(samples.iter().map(|sample| sample.cpu_ns));
    scratch.sort_unstable();
    let cpu_median_ns = required_percentile(scratch, 50)?;
    let cpu_p95_ns = required_percentile(scratch, 95)?;
    let cpu_p99_ns = required_percentile(scratch, 99)?;
    scratch.clear();
    scratch.extend(samples.iter().map(|sample| sample.frame_ns));
    scratch.sort_unstable();
    Ok(FrameSummary {
        output_count: samples.len(),
        gpu_resolved_count,
        gpu_median_ns,
        gpu_p95_ns,
        gpu_p99_ns,
        cpu_median_ns,
        cpu_p95_ns,
        cpu_p99_ns,
        frame_median_ns: required_percentile(scratch, 50)?,
        frame_p95_ns: required_percentile(scratch, 95)?,
        frame_p99_ns: required_percentile(scratch, 99)?,
        max_residency_allocation_events: maximum(samples, |s| s.residency_allocation_events),
        heap_measured_count: samples.iter().filter(|s| s.heap.is_some()).count(),
        max_heap_allocations: samples
            .iter()
            .filter_map(|s| s.heap.map(|h| h.allocations))
            .max(),
        max_heap_reallocations: samples
            .iter()
            .filter_map(|s| s.heap.map(|h| h.reallocations))
            .max(),
        max_upload_bytes: maximum(samples, |s| s.upload_bytes),
        peak_resident_bytes: maximum(samples, |s| s.resident_bytes),
        max_stall_events: maximum(samples, |s| s.stall_events),
    })
}

fn delta(counter: &'static str, previous: u64, current: u64) -> Result<u64, MetricsError> {
    current
        .checked_sub(previous)
        .ok_or(MetricsError::CounterRegression { counter })
}

fn percentile(sorted: &[u64], percentile: usize) -> Option<u64> {
    // Split the multiplication to avoid overflow even for very large slices.
    let rank = (sorted.len() / 100 * percentile
        + ((sorted.len() % 100) * percentile).div_ceil(100))
    .saturating_sub(1);
    sorted.get(rank).copied()
}

fn required_percentile(sorted: &[u64], rank: usize) -> Result<u64, MetricsError> {
    percentile(sorted, rank).ok_or(MetricsError::Empty)
}

fn maximum(samples: &[FrameSample], value: impl Fn(&FrameSample) -> u64) -> u64 {
    samples.iter().map(value).fold(0, u64::max)
}
