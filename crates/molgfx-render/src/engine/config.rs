//! Engine configuration and frame outcomes.

use super::{AdaptiveQualityConfig, DerivedCacheBudget, QualityTier, RenderProfile};
use crate::ResidencyConfig;
use molgfx_core::ResidencyBudget;
use molgfx_gpu::PowerPreference;

/// Presentation result independent of streaming completeness.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FrameStatus {
    /// The frame reached the presentation surface.
    Presented,
    /// The frame was skipped (surface lost or outdated); the surface was
    /// reconfigured and the next call recovers.
    Skipped,
}

/// Whether every requested resource contributed at full fidelity.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FrameCompleteness {
    /// No provider or upload work remains pending.
    Complete,
    /// The frame is valid but more resident detail is still arriving.
    Progressive {
        /// Bounded uploads awaiting fence completion.
        pending_chunks: u64,
    },
}

/// Allocation-free bitset describing explicit realtime degradation.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug, serde::Serialize)]
pub struct FrameDegradation(u8);

impl FrameDegradation {
    /// Non-resident detail is represented by the paged/proxy path.
    pub const STREAMING_PROXY: Self = Self(1);

    /// True when every bit in `feature` is active.
    #[must_use]
    pub const fn contains(self, feature: Self) -> bool {
        self.0 & feature.0 == feature.0
    }

    pub(super) const fn streaming_proxy(enabled: bool) -> Self {
        if enabled {
            Self::STREAMING_PROXY
        } else {
            Self(0)
        }
    }
}

/// Stable, allocation-free counters captured with a frame report.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug, serde::Serialize)]
pub struct FrameMetrics {
    /// Provider chunks tracked by the GPU residency layer.
    pub tracked_chunks: usize,
    /// Upload bytes still protected by GPU fences.
    pub upload_in_flight_bytes: u64,
    /// Presentation submissions not yet reported complete by the GPU.
    pub pending_frame_submissions: u32,
    /// Monotonic identifier for the most recently submitted frame.
    pub last_submission_id: u64,
    /// Submission timestamp in the engine monotonic clock, in nanoseconds:
    /// host time when the frame's encoder reached the queue. CPU cost of the
    /// frame is separately observable as the duration of the native `render`
    /// call; this is not device execution time.
    pub submission_timestamp_ns: u64,
    /// Host timestamp when the last submission fence was observed complete,
    /// in the same monotonic clock. `None` while pending. This is host
    /// observation latency, not exact device completion or execution time.
    /// GPU execution time requires timestamp queries via profiling.
    pub completion_timestamp_ns: Option<u64>,
    /// Retained recomputable device bytes.
    pub derived_cache_gpu_bytes: u64,
    /// Peak recomputable device bytes since engine construction.
    pub derived_cache_peak_gpu_bytes: u64,
    /// Live physical GPU buffer bytes owned through the device boundary.
    pub physical_buffer_bytes: u64,
    /// Live physical GPU texture bytes owned through the device boundary.
    pub physical_texture_bytes: u64,
    /// Total live physical GPU bytes owned through the device boundary.
    pub physical_total_bytes: u64,
    /// Peak live physical GPU bytes since device construction.
    pub physical_peak_bytes: u64,
}

/// Immutable settings of a presentation submission whose fence was observed.
#[derive(Clone, Copy, PartialEq, Debug, serde::Serialize)]
pub struct CompletedFrame {
    /// Submission identifier, independent of subsequent camera or scene edits.
    pub submission_id: u64,
    /// Settings captured at submission, with completion observed afterward.
    pub quality: super::EffectiveQuality,
}

/// Explicit frame status, completeness and degradation report.
#[derive(Clone, Copy, PartialEq, Debug, serde::Serialize)]
pub struct FrameReport {
    /// Presentation result.
    pub status: FrameStatus,
    /// Whether full requested detail was available.
    pub completeness: FrameCompleteness,
    /// Explicit approximations used by the selected mode.
    pub degradation: FrameDegradation,
    /// Residency counters captured after submission.
    pub metrics: FrameMetrics,
    /// True while temporal convergence, streaming, or surface recovery needs
    /// another caller-scheduled frame.
    pub needs_another_frame: bool,
    /// The adaptive quality tier this frame rendered at.
    pub quality_tier: QualityTier,
    /// Physical settings and exposure completion observed at reporting time.
    pub quality: super::EffectiveQuality,
    /// Latest fence-observed submission; never inferred from a later frame.
    pub last_completed: Option<CompletedFrame>,
}

/// Which rendering mode the engine runs.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum RenderMode {
    /// The interactive raster path.
    #[default]
    Realtime,
    /// Progressive, deterministic high-fidelity rendering. Camera motion
    /// resets temporal history but never changes the selected render path.
    Cinematic,
}

/// Default [`EngineConfig::surface_field_budget_bytes`]: 384 MiB, which holds a
/// field of 33 million voxels, a cube 80 Å across at 0.25 Å spacing.
pub const DEFAULT_SURFACE_FIELD_BUDGET_BYTES: u64 = 384 * 1024 * 1024;

/// Engine construction options.
#[derive(Clone, Debug)]
pub struct EngineConfig {
    /// Adapter preference.
    pub power: PowerPreference,
    /// Initial frame width, pixels.
    pub width: u32,
    /// Initial frame height, pixels.
    pub height: u32,
    /// Rendering strategy. Backend selection remains capability-driven.
    pub mode: RenderMode,
    /// Adaptive quality policy. A configuration that disables adaptation
    /// holds one tier, so converged output stays reproducible.
    pub adaptive: AdaptiveQualityConfig,
    /// Reusable presentation recipe resolved once during engine construction.
    pub profile: RenderProfile,
    /// Fixed page, staging, command and lifecycle capacities.
    pub residency: ResidencyConfig,
    /// One coordinated limit set for caller/provider-owned source data.
    pub source_budget: ResidencyBudget,
    /// Hard limits for recomputable data, separate from source residency.
    pub derived_cache: DerivedCacheBudget,
    /// Hard ceiling for live physical GPU buffers and textures.
    pub resource_memory_limit_bytes: Option<u64>,
    /// Device memory one implicit-surface field may take while it is built.
    ///
    /// A field that would not fit is sampled more coarsely, and the output
    /// reports the spacing it used; a larger budget buys finer surfaces of
    /// larger structures.
    pub surface_field_budget_bytes: u64,
    /// Maximum number of simultaneously resident dataset/namespace pick pages.
    pub picking_page_capacity: u32,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            power: PowerPreference::HighPerformance,
            width: 1280,
            height: 800,
            mode: RenderMode::Realtime,
            adaptive: AdaptiveQualityConfig::default(),
            profile: RenderProfile::inspection(),
            residency: ResidencyConfig::default(),
            source_budget: ResidencyBudget::default(),
            derived_cache: DerivedCacheBudget::default(),
            resource_memory_limit_bytes: None,
            surface_field_budget_bytes: DEFAULT_SURFACE_FIELD_BUDGET_BYTES,
            picking_page_capacity: 1_024,
        }
    }
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
