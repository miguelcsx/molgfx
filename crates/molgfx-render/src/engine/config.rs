//! Engine configuration and frame outcomes.

use super::{AdaptiveQualityConfig, DerivedCacheBudget, QualityTier, RenderProfile};
use crate::ResidencyConfig;
use molgfx_core::ResidencyBudget;
use molgfx_gpu::PowerPreference;

/// Presentation result independent of streaming completeness.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FrameStatus {
    /// The frame reached the presentation surface.
    Presented,
    /// The frame was skipped (surface lost or outdated); the surface was
    /// reconfigured and the next call recovers.
    Skipped,
}

/// Whether every requested resource contributed at full fidelity.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
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
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct FrameMetrics {
    /// Provider chunks tracked by the GPU residency layer.
    pub tracked_chunks: usize,
    /// Upload bytes still protected by GPU fences.
    pub upload_in_flight_bytes: u64,
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

/// Explicit frame status, completeness and degradation report.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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
            picking_page_capacity: 1_024,
        }
    }
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
