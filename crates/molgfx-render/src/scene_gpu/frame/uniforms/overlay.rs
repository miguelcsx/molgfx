//! Scalar-overlay values embedded in representation uniforms.

use crate::scene_gpu::ramp_lut::RampLut;
use molgfx_core::{Representation, ScalarVolume};
use molgfx_math::Mat4;
use molgfx_math::Rgba8;

pub(super) struct OverlayUniforms {
    pub(super) world_to_voxel: [[f32; 4]; 4],
    pub(super) contour: [f32; 4],
    pub(super) ramp: RampLut,
    pub(super) size: [u32; 4],
    pub(super) visual: [f32; 4],
}

pub(super) fn overlay_uniforms(
    representation: &Representation,
    volume: Option<&ScalarVolume>,
) -> OverlayUniforms {
    let (Some(style), Some(volume)) = (representation.surface_scalar, volume) else {
        return OverlayUniforms {
            world_to_voxel: Mat4::IDENTITY.to_cols_array_2d(),
            contour: [0.0; 4],
            ramp: RampLut::disabled(),
            size: [1, 1, 1, 0],
            visual: [1.0, 0.0, 0.0, 0.0],
        };
    };
    let ramp = RampLut::new(&style.ramp, Rgba8::opaque(0, 0, 0));
    let (interval, width) = match style.contours {
        Some(contours) => (
            contours.interval.max(f32::EPSILON),
            contours.width_pixels.clamp(0.25, 8.0),
        ),
        None => (0.0, 1.0),
    };
    let dimensions = volume.dimensions();
    OverlayUniforms {
        world_to_voxel: volume.voxel_to_world().inverse().to_cols_array_2d(),
        contour: [reciprocal(interval), 0.0, 0.0, 0.0],
        ramp,
        size: [dimensions[0], dimensions[1], dimensions[2], 1],
        visual: [
            width,
            if style.sample_offset_angstrom.is_finite() {
                style.sample_offset_angstrom.clamp(-100.0, 100.0)
            } else {
                0.0
            },
            0.0,
            0.0,
        ],
    }
}

/// The reciprocal of a contour interval, or zero for none.
///
/// The shader multiplies by it, so contours cost no division per fragment.
fn reciprocal(interval: f32) -> f32 {
    if interval > 0.0 { 1.0 / interval } else { 0.0 }
}
