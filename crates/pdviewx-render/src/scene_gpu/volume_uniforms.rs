//! Byte-exact direct-volume representation parameters.

use super::uniforms::{clip_meta, clip_planes, material_uniforms};
use pdviewx_core::{DensityVolume, MAX_CLIP_PLANES, MAX_VOLUME_TRANSFER_POINTS, Representation};
use pdviewx_math::{Mat4, Vec3};

#[cfg(test)]
#[path = "volume_uniforms_tests.rs"]
mod tests;

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct VolumeUniforms {
    pub(super) voxel_to_world: Mat4,
    pub(super) world_to_voxel: Mat4,
    pub(super) dimensions: [u32; 4],
    pub(super) empty_space_dimensions: [u32; 4],
    pub(super) scalar: [f32; 4],
    pub(super) sampling: [f32; 4],
    pub(super) transfer_values: [[f32; 4]; MAX_VOLUME_TRANSFER_POINTS],
    pub(super) transfer_colors: [[f32; 4]; MAX_VOLUME_TRANSFER_POINTS],
    pub(super) transfer_meta: [u32; 4],
    pub(super) clip_planes: [[f32; 4]; MAX_CLIP_PLANES],
    pub(super) clip_meta: [u32; 4],
    pub(super) crop_minimum: [u32; 4],
    pub(super) crop_maximum: [u32; 4],
    pub(super) slice_plane: [f32; 4],
    pub(super) material: [f32; 4],
}

impl VolumeUniforms {
    pub(super) fn new(volume: &DensityVolume, representation: &Representation) -> Self {
        let transform = volume.voxel_to_world();
        let axes = [
            transform.transform_vector3(Vec3::X).length(),
            transform.transform_vector3(Vec3::Y).length(),
            transform.transform_vector3(Vec3::Z).length(),
        ];
        let range = volume.range();
        let points = representation.volume.transfer.points();
        let mut transfer_values = [[0.0; 4]; MAX_VOLUME_TRANSFER_POINTS];
        let mut transfer_colors = [[0.0; 4]; MAX_VOLUME_TRANSFER_POINTS];
        for (index, point) in points.iter().enumerate() {
            transfer_values[index] = [point.value, point.opacity, 0.0, 0.0];
            transfer_colors[index] = point.color.to_f32();
        }
        let opacity = finite_or(representation.material.opacity, 1.0).clamp(0.0, 1.0);
        let density = finite_or(representation.volume.opacity_scale, 2.0).max(0.0);
        let step = finite_or(representation.volume.step_scale, 0.65).clamp(0.2, 2.0);
        let crop = representation.volume.region;
        let minimum = crop.map_or([0; 3], pdviewx_core::VolumeRegion::minimum);
        let maximum = crop.map_or(volume.dimensions(), pdviewx_core::VolumeRegion::maximum);
        Self {
            voxel_to_world: transform,
            world_to_voxel: transform.inverse(),
            dimensions: [
                volume.dimensions()[0],
                volume.dimensions()[1],
                volume.dimensions()[2],
                DensityVolume::EMPTY_SPACE_BRICK_SIZE,
            ],
            empty_space_dimensions: [
                volume.empty_space_dimensions()[0],
                volume.empty_space_dimensions()[1],
                volume.empty_space_dimensions()[2],
                0,
            ],
            scalar: [
                range[0],
                range[1],
                finite_or(representation.params.isolevel, (range[0] + range[1]) * 0.5)
                    .clamp(range[0], range[1]),
                opacity,
            ],
            sampling: [
                density * opacity,
                step,
                axes.into_iter().fold(f32::INFINITY, f32::min),
                0.0,
            ],
            transfer_values,
            transfer_colors,
            transfer_meta: [
                u32::try_from(points.len()).map_or(2, |count| count),
                representation.volume.rendering as u32,
                0,
                0,
            ],
            clip_planes: clip_planes(&representation.clipping),
            clip_meta: clip_meta(&representation.clipping),
            crop_minimum: [minimum[0], minimum[1], minimum[2], 0],
            crop_maximum: [maximum[0], maximum[1], maximum[2], 0],
            slice_plane: representation
                .volume
                .slice
                .map_or([0.0, 0.0, 1.0, 0.0], |slice| {
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

fn finite_or(value: f32, fallback: f32) -> f32 {
    if value.is_finite() { value } else { fallback }
}
