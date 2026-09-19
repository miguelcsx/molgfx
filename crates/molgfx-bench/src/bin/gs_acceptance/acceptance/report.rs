//! Machine-readable evidence without sentinel values for missing metrics.

use super::{GoldenSceneId, SceneRecipe};
use molgfx_bench::FrameSummary;
use serde::Serialize;

/// A measured value or an explicit reason why it is absent.
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub(crate) enum Availability<T> {
    Available {
        value: T,
        unit: &'static str,
        source: &'static str,
    },
    Unavailable {
        reason: String,
    },
}

/// Single-adapter identity evidence. This never implies cross-adapter coverage.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct AdapterEvidence {
    pub(crate) scope: &'static str,
    pub(crate) capability_fingerprint: String,
    pub(crate) runtime_name: Availability<String>,
}

/// Candidate image evidence. Candidates never overwrite accepted references.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct CandidateEvidence {
    pub(crate) path: String,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) stabilization_renders: usize,
    pub(crate) repeated_bytes_equal: bool,
    pub(crate) repeated_path_on_mismatch: Option<String>,
    pub(crate) accepted_reference: Availability<String>,
}

/// Real frame-loop and process metrics.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct MetricSet {
    pub(crate) measured_frames: usize,
    pub(crate) gpu_p50_ns: Availability<u64>,
    pub(crate) gpu_p95_ns: Availability<u64>,
    pub(crate) gpu_p99_ns: Availability<u64>,
    pub(crate) cpu_p50_ns: Availability<u64>,
    pub(crate) cpu_p95_ns: Availability<u64>,
    pub(crate) cpu_p99_ns: Availability<u64>,
    pub(crate) frame_p50_ns: Availability<u64>,
    pub(crate) frame_p95_ns: Availability<u64>,
    pub(crate) frame_p99_ns: Availability<u64>,
    pub(crate) max_residency_allocation_events: Availability<u64>,
    pub(crate) max_process_heap_allocations: Availability<u64>,
    pub(crate) max_upload_bytes: Availability<u64>,
    pub(crate) peak_resident_bytes: Availability<u64>,
    pub(crate) max_stall_events: Availability<u64>,
    pub(crate) peak_rss_bytes: Availability<u64>,
    pub(crate) peak_vram_bytes: Availability<u64>,
}

impl MetricSet {
    #[must_use]
    pub(crate) fn measured(
        frames: usize,
        summary: FrameSummary,
        gpu_samples_resolved: bool,
        max_process_heap_allocations: u64,
    ) -> Self {
        let timing = |value| Availability::Available {
            value,
            unit: "ns",
            source: "production FrameTiming",
        };
        let gpu_timing = |value| {
            if gpu_samples_resolved {
                timing(value)
            } else {
                Availability::Unavailable {
                    reason: "timestamp query readback did not resolve every measured frame".into(),
                }
            }
        };
        let counter = |value, unit, source| Availability::Available {
            value,
            unit,
            source,
        };
        Self {
            measured_frames: frames,
            gpu_p50_ns: gpu_timing(summary.gpu_median_ns),
            gpu_p95_ns: gpu_timing(summary.gpu_p95_ns),
            gpu_p99_ns: gpu_timing(summary.gpu_p99_ns),
            cpu_p50_ns: timing(summary.cpu_median_ns),
            cpu_p95_ns: timing(summary.cpu_p95_ns),
            cpu_p99_ns: timing(summary.cpu_p99_ns),
            frame_p50_ns: timing(summary.frame_median_ns),
            frame_p95_ns: timing(summary.frame_p95_ns),
            frame_p99_ns: timing(summary.frame_p99_ns),
            max_residency_allocation_events: counter(
                summary.max_allocations,
                "events/frame",
                "residency-owned host allocation counter",
            ),
            max_process_heap_allocations: counter(
                max_process_heap_allocations,
                "events/frame",
                "process-wide instrumented system allocator",
            ),
            max_upload_bytes: counter(
                summary.max_upload_bytes,
                "bytes/frame",
                "production upload-ring counter",
            ),
            peak_resident_bytes: counter(
                summary.peak_resident_bytes,
                "bytes",
                "production residency gauge",
            ),
            max_stall_events: counter(
                summary.max_stall_events,
                "events/frame",
                "production residency stall counter",
            ),
            peak_rss_bytes: Availability::Unavailable {
                reason: "process RSS requires an external observer of the benchmark process".into(),
            },
            peak_vram_bytes: Availability::Unavailable {
                reason: "active backend exposes resident arena bytes, not total process VRAM"
                    .into(),
            },
        }
    }

    pub(crate) fn unavailable(frames: usize, reason: &str) -> Self {
        let missing = || Availability::Unavailable {
            reason: reason.to_owned(),
        };
        Self {
            measured_frames: frames,
            gpu_p50_ns: missing(),
            gpu_p95_ns: missing(),
            gpu_p99_ns: missing(),
            cpu_p50_ns: missing(),
            cpu_p95_ns: missing(),
            cpu_p99_ns: missing(),
            frame_p50_ns: missing(),
            frame_p95_ns: missing(),
            frame_p99_ns: missing(),
            max_residency_allocation_events: missing(),
            max_process_heap_allocations: Availability::Unavailable {
                reason: "no process-wide allocation counter is installed in the benchmark binary"
                    .into(),
            },
            max_upload_bytes: missing(),
            peak_resident_bytes: missing(),
            max_stall_events: missing(),
            peak_rss_bytes: Availability::Unavailable {
                reason: "process RSS requires an external observer of the benchmark process".into(),
            },
            peak_vram_bytes: Availability::Unavailable {
                reason: "active backend exposes no total process VRAM counter".into(),
            },
        }
    }
}

/// Conservative scene outcome.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SceneOutcome {
    Candidate,
    Blocked,
    Failed,
}

/// Evidence for one GS row.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct SceneEvidence {
    pub(crate) id: GoldenSceneId,
    pub(crate) title: &'static str,
    pub(crate) recipe: SceneRecipe,
    pub(crate) outcome: SceneOutcome,
    pub(crate) fixture: Option<String>,
    pub(crate) target_atoms: u64,
    pub(crate) observed_atoms: Option<u64>,
    pub(crate) blockers: Vec<String>,
    pub(crate) candidate: Option<CandidateEvidence>,
    pub(crate) metrics: Option<MetricSet>,
}

/// One invocation's evidence envelope.
#[derive(Clone, Debug, Serialize)]
pub(crate) struct AcceptanceReport {
    pub(crate) schema_version: u32,
    pub(crate) evidence_scope: &'static str,
    pub(crate) adapter: AdapterEvidence,
    pub(crate) scenes: Vec<SceneEvidence>,
}
