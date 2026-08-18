//! Engine configuration and frame outcomes.

use super::RenderProfile;
use pdviewx_gpu::PowerPreference;

/// What a frame call produced.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FrameOutcome {
    /// The frame was rendered and presented.
    Presented,
    /// The frame was skipped (surface lost or outdated); the surface was
    /// reconfigured and the next call recovers.
    Skipped,
}

/// Which rendering mode the engine runs.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum RenderMode {
    /// The interactive raster path.
    #[default]
    Realtime,
    /// Progressive BVH-traced cavity occlusion and area-light shadows while
    /// the camera is still, with automatic realtime fallback during motion.
    Quality,
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
    /// Reusable presentation recipe resolved once during engine construction.
    pub profile: RenderProfile,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            power: PowerPreference::HighPerformance,
            width: 1280,
            height: 800,
            mode: RenderMode::Realtime,
            profile: RenderProfile::inspection(),
        }
    }
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
