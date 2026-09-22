//! Scalar-overlay values embedded in representation uniforms.

use molgfx_core::{Representation, ScalarVolume};
use molgfx_math::Mat4;

pub(super) struct OverlayUniforms {
    pub(super) world_to_voxel: [[f32; 4]; 4],
    pub(super) domain: [f32; 4],
    pub(super) colors: [[f32; 4]; 3],
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
            domain: [0.0, 0.5, 1.0, 0.0],
            colors: [[0.0; 4]; 3],
            size: [1, 1, 1, 0],
            visual: [1.0, 0.0, 0.0, 0.0],
        };
    };
    let values = style.ramp.values();
    let colors = style.ramp.colors().map(color_f32);
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
        domain: [values[0], values[1], values[2], interval],
        colors,
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

fn color_f32(color: molgfx_math::Rgba8) -> [f32; 4] {
    let scale = 1.0 / 255.0;
    [
        f32::from(color.r) * scale,
        f32::from(color.g) * scale,
        f32::from(color.b) * scale,
        f32::from(color.a) * scale,
    ]
}
