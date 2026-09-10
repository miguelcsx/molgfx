//! Compact GPU records and checked indirect ranges for ligand pose batches.
//!
//! A candidate occupies thirty-six bytes across transform and style columns.
//! Topology is stored once per batch, and one indirect draw expands the
//! Cartesian product of a pose range and an atom or bond topology range. Pose
//! offsets name two globally partitioned physical columns: opaque first and
//! translucent second.

use crate::error::RenderError;
use pdviewx_core::{DrawIndirectArgs, EntityId, EntityKind, LigandPose};
use pdviewx_math::{Mat4, Quat, Rgba8, Vec3};

pub(crate) const POSE_SPHERE: u32 = 0;
pub(crate) const POSE_CAPSULE: u32 = 3;
pub(super) const QUAD_VERTICES: u32 = 6;
pub(super) const POSE_HASH_OFFSET: u64 = 14_695_981_039_346_656_037;

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct PoseTransformGpu {
    pub(super) translation_opacity: [f32; 4],
    pub(super) orientation: [f32; 4],
}

impl From<LigandPose> for PoseTransformGpu {
    fn from(pose: LigandPose) -> Self {
        let translation = pose.translation();
        let orientation = pose.orientation();
        Self {
            translation_opacity: [translation.x, translation.y, translation.z, pose.opacity()],
            orientation: orientation.to_array(),
        }
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct PoseStyleGpu(pub(super) u32);

impl From<LigandPose> for PoseStyleGpu {
    fn from(pose: LigandPose) -> Self {
        Self(pack_color(pose.color()))
    }
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct PoseBondGpu {
    pub(super) center_length: [f32; 4],
    pub(super) orientation: [f32; 4],
}

impl PoseBondGpu {
    pub(super) fn new(center: Vec3, orientation: Quat, length: f32) -> Self {
        Self {
            center_length: [center.x, center.y, center.z, length],
            orientation: orientation.to_array(),
        }
    }
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct PoseBatchGpu {
    pub(super) topology: [u32; 4],
    pub(super) poses: [u32; 4],
    pub(super) sampling: [u32; 4],
    pub(super) radii: [f32; 4],
    pub(super) model: [[f32; 4]; 4],
}

impl PoseBatchGpu {
    pub(super) fn new(
        topology: [u32; 4],
        poses: [u32; 2],
        sampling: [u32; 4],
        entity_id: EntityId,
        pick_page: u32,
        radii: [f32; 2],
        model: Mat4,
    ) -> Self {
        Self {
            topology,
            poses: [poses[0], poses[1], entity_id.0, pick_page],
            sampling,
            radii: [radii[0] * 2.0, radii[1] * 2.0, 0.0, 0.0],
            model: model.to_cols_array_2d(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct LigandPoseDrawGroup {
    pub(crate) shape: u32,
    pub(crate) translucent: bool,
    pub(crate) args_offset: u64,
    pub(crate) instances: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct PoseTableStats {
    pub(crate) resident_poses: u64,
    pub(crate) visible_poses: u64,
    pub(crate) selected_poses: u64,
    pub(crate) beauty_instances: u64,
    pub(crate) shadow_instances: u64,
    pub(crate) draw_groups: u32,
}

pub(super) struct BatchRanges {
    pub(super) opaque: u32,
    pub(super) translucent: u32,
    pub(super) stable_key: u64,
}

impl BatchRanges {
    pub(super) fn visible(&self) -> Result<u32, RenderError> {
        checked_add(self.opaque, self.translucent, "visible ligand pose count")
    }
}

#[derive(Clone, Copy)]
pub(super) struct PoseDrawRange {
    pub(super) batch_index: u32,
    pub(super) shape: u32,
    pub(super) topology_count: u32,
    pub(super) pose_count: u32,
    pub(super) translucent: bool,
}

pub(super) fn classify_poses(
    poses: &[LigandPose],
    mut stable_key: u64,
    opaque_indices: &mut Vec<u32>,
    translucent_indices: &mut Vec<u32>,
) -> Result<BatchRanges, RenderError> {
    let opaque_first = opaque_indices.len();
    let translucent_first = translucent_indices.len();
    for (index, pose) in poses.iter().copied().enumerate() {
        let index = checked_count(index, "ligand pose source index")?;
        hash_pose(&mut stable_key, pose);
        let alpha = pose_final_alpha(pose);
        if alpha <= 0.0 {
            continue;
        }
        if alpha < 1.0 {
            translucent_indices.push(index);
        } else {
            opaque_indices.push(index);
        }
    }
    Ok(BatchRanges {
        opaque: checked_count(
            opaque_indices.len() - opaque_first,
            "opaque ligand pose count",
        )?,
        translucent: checked_count(
            translucent_indices.len() - translucent_first,
            "translucent ligand pose count",
        )?,
        stable_key,
    })
}

pub(super) fn append_draw(
    args: &mut Vec<DrawIndirectArgs>,
    groups: &mut Vec<LigandPoseDrawGroup>,
    range: PoseDrawRange,
) -> Result<(), RenderError> {
    let PoseDrawRange {
        batch_index,
        shape,
        topology_count,
        pose_count,
        translucent,
    } = range;
    if topology_count == 0 || pose_count == 0 {
        return Ok(());
    }
    let first_vertex = checked_mul(batch_index, QUAD_VERTICES, "ligand pose batch index")?;
    let instance_count = checked_mul(pose_count, topology_count, "ligand pose draw instances")?;
    let args_offset = checked_bytes::<DrawIndirectArgs>(args.len(), "ligand pose draw arguments")?;
    args.push(DrawIndirectArgs {
        vertex_count: QUAD_VERTICES,
        instance_count,
        first_vertex,
        first_instance: 0,
    });
    groups.push(LigandPoseDrawGroup {
        shape,
        translucent,
        args_offset,
        instances: instance_count,
    });
    Ok(())
}

pub(super) fn checked_count(value: usize, resource: &'static str) -> Result<u32, RenderError> {
    u32::try_from(value).map_err(|_| limit(resource, u64::from(u32::MAX)))
}

pub(super) fn checked_add(
    left: u32,
    right: u32,
    resource: &'static str,
) -> Result<u32, RenderError> {
    left.checked_add(right)
        .ok_or_else(|| limit(resource, u64::from(u32::MAX)))
}

pub(super) fn checked_mul(
    left: u32,
    right: u32,
    resource: &'static str,
) -> Result<u32, RenderError> {
    left.checked_mul(right)
        .ok_or_else(|| limit(resource, u64::from(u32::MAX)))
}

pub(super) fn checked_bytes<T>(count: usize, resource: &'static str) -> Result<u64, RenderError> {
    let count = u64::try_from(count).map_err(|_| limit(resource, u64::MAX))?;
    count
        .checked_mul(std::mem::size_of::<T>() as u64)
        .ok_or_else(|| limit(resource, u64::MAX))
}

#[cfg(test)]
pub(super) fn pose_is_translucent(pose: LigandPose) -> bool {
    let alpha = pose_final_alpha(pose);
    alpha > 0.0 && alpha < 1.0
}

#[cfg(test)]
pub(super) fn pose_is_visible(pose: LigandPose) -> bool {
    pose_final_alpha(pose) > 0.0
}

fn pose_final_alpha(pose: LigandPose) -> f32 {
    pose.color().to_f32()[3] * pose.opacity()
}

pub(super) fn mix_pose_key(hash: &mut u64, value: u32) {
    *hash ^= u64::from(value);
    *hash = (*hash).wrapping_mul(1_099_511_628_211);
}

fn hash_pose(hash: &mut u64, pose: LigandPose) {
    for value in pose.translation().to_array() {
        mix_pose_key(hash, value.to_bits());
    }
    for value in pose.orientation().to_array() {
        mix_pose_key(hash, value.to_bits());
    }
    let color = pose.color();
    mix_pose_key(
        hash,
        u32::from(color.r)
            | (u32::from(color.g) << 8)
            | (u32::from(color.b) << 16)
            | (u32::from(color.a) << 24),
    );
    mix_pose_key(hash, pose.opacity().to_bits());
}

pub(super) fn batch_entity(row: u32) -> Result<EntityId, pdviewx_core::EntityIdError> {
    EntityId::pack(EntityKind::LigandPoseBatch, u64::from(row))
}

const fn pack_color(color: Rgba8) -> u32 {
    color.r as u32 | ((color.g as u32) << 8) | ((color.b as u32) << 16) | ((color.a as u32) << 24)
}

fn limit(resource: &'static str, limit: u64) -> RenderError {
    pdviewx_gpu::GpuError::LimitExceeded { resource, limit }.into()
}

#[cfg(test)]
#[path = "ligand_pose_types_tests.rs"]
mod tests;
