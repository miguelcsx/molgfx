//! Byte-exact categorical-volume representation parameters.

use super::segmentation_lookup::{LookupMode, SegmentLookup};
use super::uniforms::{clip_meta, clip_planes, material_uniforms};
use molgfx_core::{MAX_CLIP_PLANES, Representation, SegmentedVolume};
use molgfx_math::Mat4;

#[cfg(test)]
#[path = "segmentation_uniforms_tests.rs"]
mod tests;

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct SegmentationUniforms {
    pub(super) voxel_to_world: Mat4,
    pub(super) world_to_voxel: Mat4,
    pub(super) dimensions: [u32; 4],
    pub(super) sampling: [f32; 4],
    pub(super) lookup: [u32; 4],
    pub(super) clip_planes: [[f32; 4]; MAX_CLIP_PLANES],
    pub(super) clip_meta: [u32; 4],
    pub(super) crop_minimum: [u32; 4],
    pub(super) crop_maximum: [u32; 4],
    pub(super) slice_plane: [f32; 4],
    pub(super) material: [f32; 4],
}

impl SegmentationUniforms {
    pub(super) fn new(
        volume: &SegmentedVolume,
        representation: &Representation,
        source_id: u32,
        lookup: &SegmentLookup,
    ) -> Self {
        let transform = volume.voxel_to_world();
        let style = &representation.segmentation;
        let dimensions = volume.dimensions();
        let minimum = style
            .region
            .map_or([0; 3], molgfx_core::VolumeRegion::minimum);
        let maximum = style
            .region
            .map_or(dimensions, molgfx_core::VolumeRegion::maximum);
        let opacity = style.opacity_scale;
        let mode = match lookup.mode() {
            LookupMode::Direct => 0,
            LookupMode::Hash => 1,
        };
        Self {
            voxel_to_world: transform,
            world_to_voxel: transform.inverse(),
            dimensions: [
                dimensions[0],
                dimensions[1],
                dimensions[2],
                u32::from(style.region.is_some()),
            ],
            sampling: [
                opacity,
                0.0,
                transform.minimum_axis_length(),
                if style.slice.is_some() { 1.0 } else { 0.0 },
            ],
            lookup: [
                mode,
                crate::fallback(u32::try_from(lookup.entries().len()), u32::MAX),
                lookup.max_label(),
                source_id,
            ],
            clip_planes: clip_planes(&representation.clipping),
            clip_meta: clip_meta(&representation.clipping),
            crop_minimum: [minimum[0], minimum[1], minimum[2], 0],
            crop_maximum: [maximum[0], maximum[1], maximum[2], 0],
            slice_plane: style.slice.map_or([0.0, 0.0, 1.0, 0.0], |slice| {
                [
                    slice.plane.normal.x,
                    slice.plane.normal.y,
                    slice.plane.normal.z,
                    slice.plane.offset,
                ]
            }),
            material: material_uniforms(representation.material),
        }
    }
}
