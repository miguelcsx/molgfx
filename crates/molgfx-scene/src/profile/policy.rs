//! Public renderer policy and its named recipes.

use super::DepthCue;

/// Adaptive or fixed quality policy.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Quality {
    /// Adapt expensive effects to the requested frame budget.
    #[default]
    Auto,
    /// Hold a reduced detail tier for low interactive latency.
    Interactive,
    /// Hold maximum detail regardless of scene size or frame time.
    HighestFixed,
    /// Maximum detail for converged output.
    Converged,
}

/// Public renderer policy without target dimensions or backend details.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct RenderProfile {
    /// Desired frame rate; fixed policies treat this as a target, not a detail cap.
    pub target_fps: u16,
    /// Quality policy.
    pub quality: Quality,
    /// Optional explicit view-space depth cue.
    pub depth_cue: Option<DepthCue>,
    /// Optional explicit edge-smoothing choice.
    ///
    /// Left unset, realtime edges are smoothed while converged accumulation
    /// already averages sub-pixel coverage.
    pub edge_smoothing: Option<bool>,
}

impl Default for RenderProfile {
    fn default() -> Self {
        interactive()
    }
}

impl RenderProfile {
    /// Enables one validated depth cue without changing quality policy.
    #[must_use]
    pub const fn with_depth_cue(mut self, depth_cue: DepthCue) -> Self {
        self.depth_cue = Some(depth_cue);
        self
    }

    /// Sets edge smoothing explicitly, overriding the tier default.
    #[must_use]
    pub const fn with_edge_smoothing(mut self, edge_smoothing: bool) -> Self {
        self.edge_smoothing = Some(edge_smoothing);
        self
    }

    /// Removes the explicit depth cue.
    #[must_use]
    pub const fn without_depth_cue(mut self) -> Self {
        self.depth_cue = None;
        self
    }
}

/// Interactive adaptive profile.
#[must_use]
pub const fn interactive() -> RenderProfile {
    adaptive(60)
}

/// Converged converged profile with maximum fixed detail.
#[must_use]
pub const fn converged() -> RenderProfile {
    recipe(1, Quality::Converged)
}

/// Adaptive profile targeting a caller-selected refresh rate.
#[must_use]
pub const fn adaptive(target_fps: u16) -> RenderProfile {
    recipe(target_fps, Quality::Auto)
}

/// Maximum fixed detail targeting a refresh rate without adapting to meet it.
#[must_use]
pub const fn highest_fixed(target_fps: u16) -> RenderProfile {
    recipe(target_fps, Quality::HighestFixed)
}

const fn recipe(target_fps: u16, quality: Quality) -> RenderProfile {
    let target_fps = if target_fps == 0 {
        1
    } else if target_fps > 1_000 {
        1_000
    } else {
        target_fps
    };
    RenderProfile {
        target_fps,
        quality,
        depth_cue: None,
        edge_smoothing: None,
    }
}
