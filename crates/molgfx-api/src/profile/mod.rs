//! Small renderer policy values.

/// Adaptive or fixed quality policy.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Quality {
    /// Adapt expensive effects to the requested frame budget.
    #[default]
    Auto,
    /// Favor low interactive latency.
    Interactive,
    /// Favor converged publication output.
    Publication,
}

/// Public renderer policy without target dimensions or backend details.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct RenderProfile {
    /// Desired frame rate used by adaptive quality.
    pub target_fps: u16,
    /// Quality policy.
    pub quality: Quality,
}

impl Default for RenderProfile {
    fn default() -> Self {
        Self {
            target_fps: 60,
            quality: Quality::Auto,
        }
    }
}

/// Interactive adaptive profile.
#[must_use]
pub const fn interactive() -> RenderProfile {
    RenderProfile {
        target_fps: 60,
        quality: Quality::Auto,
    }
}

/// Converged publication profile.
#[must_use]
pub const fn publication() -> RenderProfile {
    RenderProfile {
        target_fps: 1,
        quality: Quality::Publication,
    }
}

/// Adaptive profile targeting a caller-selected refresh rate.
#[must_use]
pub const fn adaptive(target_fps: u16) -> RenderProfile {
    let target_fps = if target_fps == 0 {
        1
    } else if target_fps > 1_000 {
        1_000
    } else {
        target_fps
    };
    RenderProfile {
        target_fps,
        quality: Quality::Auto,
    }
}

#[cfg(test)]
mod tests;
