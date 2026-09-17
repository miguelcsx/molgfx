//! Shared render-target formats and resource identifiers.

use molgfx_gpu::{BlendMode, ColorTarget, TextureFormat};

pub(crate) const AO_FORMAT: TextureFormat = TextureFormat::Rgba8Unorm;
pub(crate) const SHADOW_RESOURCE: crate::graph::ResourceId = crate::graph::ResourceId(21);
pub(crate) const GBUFFER_ALBEDO_FORMAT: TextureFormat = TextureFormat::Rgba16Float;
pub(crate) const GBUFFER_NORMAL_FORMAT: TextureFormat = TextureFormat::Rgba16Float;
pub(crate) const GBUFFER_MOTION_FORMAT: TextureFormat = TextureFormat::Rg16Float;

pub(crate) const fn gbuffer_targets() -> [ColorTarget; 5] {
    [
        ColorTarget {
            format: GBUFFER_ALBEDO_FORMAT,
            blend: BlendMode::Replace,
        },
        ColorTarget {
            format: GBUFFER_NORMAL_FORMAT,
            blend: BlendMode::Replace,
        },
        ColorTarget {
            format: TextureFormat::R32Uint,
            blend: BlendMode::Replace,
        },
        ColorTarget {
            format: TextureFormat::R32Uint,
            blend: BlendMode::Replace,
        },
        ColorTarget {
            format: GBUFFER_MOTION_FORMAT,
            blend: BlendMode::Replace,
        },
    ]
}

/// The categorical OIT pipeline writes color accumulation, revealage, source
/// volume identity and the full caller label in that order.
pub(crate) const fn segmentation_targets() -> [ColorTarget; 4] {
    [
        ColorTarget {
            format: TextureFormat::Rgba16Float,
            blend: BlendMode::Additive,
        },
        ColorTarget {
            format: TextureFormat::R8Unorm,
            blend: BlendMode::ReverseMultiply,
        },
        ColorTarget {
            format: TextureFormat::R32Uint,
            blend: BlendMode::Replace,
        },
        ColorTarget {
            format: TextureFormat::R32Uint,
            blend: BlendMode::Replace,
        },
    ]
}

/// Shared opaque albedo plus compact material class.
pub(crate) const ALBEDO_RESOURCE: crate::graph::ResourceId = crate::graph::ResourceId(1);
/// View-space normal plus perceptual roughness.
pub(crate) const NORMAL_RESOURCE: crate::graph::ResourceId = crate::graph::ResourceId(2);
/// Chunk-local row written by the visible fragment.
pub(crate) const ENTITY_RESOURCE: crate::graph::ResourceId = crate::graph::ResourceId(3);
/// Resident picking page paired with the chunk-local row.
pub(crate) const STRUCTURE_RESOURCE: crate::graph::ResourceId = crate::graph::ResourceId(4);
/// Source categorical-volume slot written by the segmentation OIT pass.
pub(crate) const SEGMENT_VOLUME_RESOURCE: crate::graph::ResourceId = crate::graph::ResourceId(19);
/// Full caller-supplied categorical label written by the segmentation OIT pass.
pub(crate) const SEGMENT_LABEL_RESOURCE: crate::graph::ResourceId = crate::graph::ResourceId(20);
/// Screen-space ambient visibility.
pub(crate) const AO_RESOURCE: crate::graph::ResourceId = crate::graph::ResourceId(5);
/// Linear HDR lighting result.
pub(crate) const HDR_RESOURCE: crate::graph::ResourceId = crate::graph::ResourceId(6);
/// First ping-pong HDR and depth history.
pub(crate) const HISTORY_A_RESOURCE: crate::graph::ResourceId = crate::graph::ResourceId(7);
/// Second ping-pong HDR and depth history.
pub(crate) const HISTORY_B_RESOURCE: crate::graph::ResourceId = crate::graph::ResourceId(8);
/// Weighted-blended translucent color and alpha accumulation.
pub(crate) const OIT_ACCUM_RESOURCE: crate::graph::ResourceId = crate::graph::ResourceId(9);
/// Product of inverse translucent coverage.
pub(crate) const OIT_REVEAL_RESOURCE: crate::graph::ResourceId = crate::graph::ResourceId(10);
/// Opaque plus translucent linear HDR, consumed by temporal resolve.
pub(crate) const COMPOSITE_RESOURCE: crate::graph::ResourceId = crate::graph::ResourceId(11);
/// Per-16×16-tile maximum circle of confusion.
pub(crate) const DOF_TILE_RESOURCE: crate::graph::ResourceId = crate::graph::ResourceId(12);
/// Linear HDR after the depth-of-field gather.
pub(crate) const DOF_RESOURCE: crate::graph::ResourceId = crate::graph::ResourceId(13);
/// Previous-UV minus current-UV for the nearest opaque surface.
pub(crate) const MOTION_RESOURCE: crate::graph::ResourceId = crate::graph::ResourceId(14);
/// Occlusion and contact shadow after edge-aware denoising.
pub(crate) const AO_DENOISED_RESOURCE: crate::graph::ResourceId = crate::graph::ResourceId(18);
/// Quarter-resolution above-threshold energy from the bright pass.
pub(crate) const BLOOM_A_RESOURCE: crate::graph::ResourceId = crate::graph::ResourceId(15);
/// Quarter-resolution horizontally blurred bloom.
pub(crate) const BLOOM_B_RESOURCE: crate::graph::ResourceId = crate::graph::ResourceId(16);
/// Quarter-resolution finished bloom the tonemap composites. Writing the final
/// blur to its own target keeps the dataflow acyclic; its lifetime does not
/// overlap the bright pass, so the pool aliases both onto one texture.
pub(crate) const BLOOM_C_RESOURCE: crate::graph::ResourceId = crate::graph::ResourceId(17);
/// Full-resolution HDR after camera-shutter motion blur.
pub(crate) const MOTION_BLUR_RESOURCE: crate::graph::ResourceId = crate::graph::ResourceId(22);

#[cfg(test)]
#[path = "resources_tests.rs"]
mod tests;
