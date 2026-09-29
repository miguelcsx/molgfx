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

/// Validated view-space depth cue applied after lighting.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct DepthCue {
    near_distance: f32,
    far_distance: f32,
    strength: f32,
}

impl DepthCue {
    /// Creates a cue with distances in scene units and a backdrop blend in
    /// `[0, 1]`.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::InvalidSpec`] when values are non-finite,
    /// negative, out of range, or not strictly ordered.
    pub fn new(near_distance: f32, far_distance: f32, strength: f32) -> Result<Self, crate::Error> {
        if !near_distance.is_finite()
            || !far_distance.is_finite()
            || !strength.is_finite()
            || near_distance < 0.0
            || far_distance <= near_distance
            || !(0.0..=1.0).contains(&strength)
        {
            return Err(crate::Error::InvalidSpec(
                "depth cue requires finite non-negative near distance, a larger far distance, and strength in [0, 1]"
                    .to_owned(),
            ));
        }
        Ok(Self {
            near_distance,
            far_distance,
            strength,
        })
    }

    /// Distance where the cue begins.
    #[must_use]
    pub const fn near_distance(self) -> f32 {
        self.near_distance
    }

    /// Distance where the cue reaches full strength.
    #[must_use]
    pub const fn far_distance(self) -> f32 {
        self.far_distance
    }

    /// Blend toward the resolved backdrop.
    #[must_use]
    pub const fn strength(self) -> f32 {
        self.strength
    }
}

/// Public renderer policy without target dimensions or backend details.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct RenderProfile {
    /// Desired frame rate used by adaptive quality.
    pub target_fps: u16,
    /// Quality policy.
    pub quality: Quality,
    /// Optional explicit view-space depth cue.
    pub depth_cue: Option<DepthCue>,
    /// Optional explicit edge-smoothing choice.
    ///
    /// Left unset, the engine smooths edges on the realtime tiers and leaves
    /// the converged tiers alone, because accumulation already averages
    /// sub-pixel coverage.
    pub edge_smoothing: Option<bool>,
}

impl Default for RenderProfile {
    fn default() -> Self {
        Self {
            target_fps: 60,
            quality: Quality::Auto,
            depth_cue: None,
            edge_smoothing: None,
        }
    }
}

impl RenderProfile {
    /// Enables one validated depth cue without changing quality policy.
    #[must_use]
    pub const fn with_depth_cue(mut self, depth_cue: DepthCue) -> Self {
        self.depth_cue = Some(depth_cue);
        self
    }

    /// Sets the edge-smoothing choice explicitly, overriding the tier default.
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
    RenderProfile {
        target_fps: 60,
        quality: Quality::Auto,
        depth_cue: None,
        edge_smoothing: None,
    }
}

/// Converged publication profile.
#[must_use]
pub const fn publication() -> RenderProfile {
    RenderProfile {
        target_fps: 1,
        quality: Quality::Publication,
        depth_cue: None,
        edge_smoothing: None,
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
        depth_cue: None,
        edge_smoothing: None,
    }
}

#[cfg(test)]
mod tests;
