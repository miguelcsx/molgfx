//! Lowers facade quality policy into one engine controller configuration.

use crate::{Quality, RenderProfile};
use molgfx_render::{AdaptiveQualityConfig, QualityTier};

pub(super) fn engine_config(
    profile: RenderProfile,
    surface_field_budget_bytes: Option<u64>,
) -> molgfx_render::EngineConfig {
    let mut presentation = match profile.quality {
        Quality::Converged => molgfx_render::RenderProfile::shape_cues(),
        Quality::Auto | Quality::Interactive | Quality::HighestFixed => {
            molgfx_render::RenderProfile::bare()
        }
    };
    presentation = profile.effects.apply(presentation);
    let adaptive = match profile.quality {
        Quality::Auto => AdaptiveQualityConfig::interactive(profile.target_fps),
        Quality::Interactive => {
            AdaptiveQualityConfig::fixed(profile.target_fps, QualityTier::Reduced)
        }
        Quality::HighestFixed | Quality::Converged => {
            AdaptiveQualityConfig::highest_fixed(profile.target_fps)
        }
    };
    molgfx_render::EngineConfig {
        mode: if profile.quality == Quality::Converged {
            molgfx_render::RenderMode::Converged
        } else {
            molgfx_render::RenderMode::Realtime
        },
        profile: presentation,
        adaptive,
        surface_field_budget_bytes: match surface_field_budget_bytes {
            Some(bytes) => bytes,
            None => molgfx_render::DEFAULT_SURFACE_FIELD_BUDGET_BYTES,
        },
        ..molgfx_render::EngineConfig::default()
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "engine_config_tests.rs"]
mod tests;
