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
    if let Some(cue) = profile.depth_cue {
        presentation = presentation.with_effect(molgfx_render::PresentationEffect::DepthCue(
            molgfx_render::DepthCue {
                near_distance: cue.near_distance(),
                far_distance: cue.far_distance(),
                strength: cue.strength(),
            },
        ));
    }
    if let Some(edge_smoothing) = profile.edge_smoothing {
        presentation = presentation.with_effect(molgfx_render::PresentationEffect::AntiAliasing(
            molgfx_render::AntiAliasingStyle { edge_smoothing },
        ));
    }
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
