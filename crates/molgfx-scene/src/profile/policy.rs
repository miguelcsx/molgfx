//! Public renderer policy and its named recipes.

use super::{Effect, EffectKind, EffectSet};
use crate::Error;

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
    /// Optional, validated presentation overrides.
    pub effects: EffectSet,
}

impl Default for RenderProfile {
    fn default() -> Self {
        interactive()
    }
}

impl RenderProfile {
    /// Replaces one effect without changing the quality policy.
    ///
    /// # Errors
    /// Returns `InvalidSpec` if an effect has invalid numeric settings.
    pub fn with_effect(mut self, effect: Effect) -> Result<Self, Error> {
        self.effects = self.effects.with(effect.validate()?);
        Ok(self)
    }

    /// Removes an explicit effect, exposing the quality recipe beneath it.
    #[must_use]
    pub fn without_effect(mut self, kind: EffectKind) -> Self {
        self.effects = self.effects.without(kind);
        self
    }

    /// Returns only the explicit override, not the quality recipe default.
    #[must_use]
    pub const fn effect(self, kind: EffectKind) -> Option<Effect> {
        self.effects.effect(kind)
    }
}

/// Interactive adaptive profile.
#[must_use]
pub const fn interactive() -> RenderProfile {
    adaptive(60)
}

/// Converged profile with maximum fixed detail.
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
        effects: EffectSet::empty(),
    }
}
