//! GPU-native reusable-topology ligand pose batches.
//!
//! Resident memory is `O(topology + poses + batches)`: the table never stores
//! the Cartesian expansion of candidates into atoms and bonds. Synchronizing a
//! changed scene uses bounded staging; stable frames issue grouped indirect
//! draws without uploads or allocations.

use super::ligand_pose_plan::{BatchPlan, PosePlanScratch, plan_pose_batches};
use super::ligand_pose_sampling::{PoseBatchSample, realtime_shadow_instances};
use super::ligand_pose_types::{
    LigandPoseDrawGroup, POSE_CAPSULE, POSE_SPHERE, PoseBatchGpu, PoseBondGpu, PoseDrawRange,
    PoseStyleGpu, PoseTableStats, PoseTransformGpu, append_draw, checked_bytes, checked_count,
};
use super::ligand_pose_upload::{PoseTableBuffers, PoseTableScratch, upload_batch_tables};
use super::primitive_packing::placement_revision;
use super::structure::GpuStructure;
use crate::error::RenderError;
use molgfx_core::{DrawIndirectArgs, Scene};
use molgfx_gpu::{BindGroupDesc, BindGroupEntry, BufferDesc, BufferUsage, Device, Queue};

const RETAINED_BUFFER_SLACK: u64 = 8 * 1024 * 1024;

#[derive(Debug)]
pub(super) struct GpuLigandPoses<D: Device> {
    atoms: Option<D::Buffer>,
    bonds: Option<D::Buffer>,
    transforms: Option<D::Buffer>,
    styles: Option<D::Buffer>,
    batches: Option<D::Buffer>,
    args: Option<D::Buffer>,
    capacities: [u64; 6],
    group: Option<D::BindGroup>,
    groups: Vec<LigandPoseDrawGroup>,
    plans: Vec<BatchPlan>,
    batch_scratch: Vec<PoseBatchGpu>,
    args_scratch: Vec<DrawIndirectArgs>,
    atom_scratch: Vec<[f32; 4]>,
    bond_scratch: Vec<PoseBondGpu>,
    transform_scratch: Vec<PoseTransformGpu>,
    style_scratch: Vec<PoseStyleGpu>,
    sample_scratch: Vec<PoseBatchSample>,
    opaque_index_scratch: Vec<u32>,
    translucent_index_scratch: Vec<u32>,
    opaque_instances: u64,
    translucent: bool,
    stats: PoseTableStats,
    synced: Option<(u64, u64, u64, bool)>,
}

impl<D: Device> GpuLigandPoses<D> {
    pub(super) const fn new() -> Self {
        Self {
            atoms: None,
            bonds: None,
            transforms: None,
            styles: None,
            batches: None,
            args: None,
            capacities: [0; 6],
            group: None,
            groups: Vec::new(),
            plans: Vec::new(),
            batch_scratch: Vec::new(),
            args_scratch: Vec::new(),
            atom_scratch: Vec::new(),
            bond_scratch: Vec::new(),
            transform_scratch: Vec::new(),
            style_scratch: Vec::new(),
            sample_scratch: Vec::new(),
            opaque_index_scratch: Vec::new(),
            translucent_index_scratch: Vec::new(),
            opaque_instances: 0,
            translucent: false,
            stats: PoseTableStats {
                resident_poses: 0,
                visible_poses: 0,
                selected_poses: 0,
                beauty_instances: 0,
                shadow_instances: 0,
                draw_groups: 0,
            },
            synced: None,
        }
    }

    pub(super) fn sync(
        &mut self,
        device: &D,
        queue: &D::Queue,
        layout: &D::BindGroupLayout,
        scene: &Scene,
        structures: &[GpuStructure<D>],
        quality: bool,
    ) -> Result<bool, RenderError> {
        let revision = (
            scene.primitive_revision(),
            scene.structure_revision(),
            placement_revision(scene),
            quality,
        );
        if self.synced == Some(revision) {
            return Ok(false);
        }
        self.plan(scene, structures, quality)?;
        if self.plans.is_empty() {
            self.release();
            self.synced = Some(revision);
            return Ok(true);
        }
        let changed = self.prepare_buffers(device)?;
        let (Some(atoms), Some(bonds), Some(transforms), Some(styles), Some(batches), Some(args)) = (
            &self.atoms,
            &self.bonds,
            &self.transforms,
            &self.styles,
            &self.batches,
            &self.args,
        ) else {
            return Ok(false);
        };
        queue.write_buffer(batches, 0, bytemuck::cast_slice(&self.batch_scratch));
        queue.write_buffer(args, 0, bytemuck::cast_slice(&self.args_scratch));
        upload_batch_tables::<D>(
            queue,
            &PoseTableBuffers::<D> {
                atoms,
                bonds,
                transforms,
                styles,
            },
            PoseTableScratch {
                atoms: &mut self.atom_scratch,
                bonds: &mut self.bond_scratch,
                transforms: &mut self.transform_scratch,
                styles: &mut self.style_scratch,
            },
            scene,
            &self.plans,
            &self.opaque_index_scratch,
            &self.translucent_index_scratch,
        )?;
        if changed || self.group.is_none() {
            self.group = Some(device.create_bind_group(&BindGroupDesc {
                label: "group2: compact ligand poses",
                layout,
                entries: &[
                    BindGroupEntry::Buffer {
                        binding: 7,
                        buffer: atoms,
                    },
                    BindGroupEntry::Buffer {
                        binding: 8,
                        buffer: bonds,
                    },
                    BindGroupEntry::Buffer {
                        binding: 9,
                        buffer: transforms,
                    },
                    BindGroupEntry::Buffer {
                        binding: 10,
                        buffer: styles,
                    },
                    BindGroupEntry::Buffer {
                        binding: 11,
                        buffer: batches,
                    },
                ],
            }));
        }
        self.synced = Some(revision);
        Ok(true)
    }

    fn plan(
        &mut self,
        scene: &Scene,
        structures: &[GpuStructure<D>],
        quality: bool,
    ) -> Result<(), RenderError> {
        self.args_scratch.clear();
        self.groups.clear();
        self.opaque_instances = 0;
        self.translucent = false;
        let source = plan_pose_batches(
            scene,
            structures,
            quality,
            PosePlanScratch {
                plans: &mut self.plans,
                batches: &mut self.batch_scratch,
                samples: &mut self.sample_scratch,
                opaque_indices: &mut self.opaque_index_scratch,
                translucent_indices: &mut self.translucent_index_scratch,
            },
        )?;
        self.stats = PoseTableStats {
            resident_poses: source.resident,
            visible_poses: source.visible,
            ..PoseTableStats::default()
        };
        self.append_class(POSE_SPHERE, false)?;
        self.append_class(POSE_CAPSULE, false)?;
        self.append_class(POSE_SPHERE, true)?;
        self.append_class(POSE_CAPSULE, true)?;
        self.finish_stats(quality)?;
        Ok(())
    }

    fn finish_stats(&mut self, quality: bool) -> Result<(), RenderError> {
        self.stats.selected_poses = self.plans.iter().try_fold(0u64, |total, plan| {
            add_u64(
                total,
                u64::from(plan.sampled_opaque) + u64::from(plan.sampled_translucent),
                "selected ligand poses",
            )
        })?;
        self.stats.beauty_instances = self.groups.iter().try_fold(0u64, |total, group| {
            add_u64(
                total,
                u64::from(group.instances),
                "ligand pose beauty instances",
            )
        })?;
        let shadows = self.opaque_instances > 0
            && (quality || realtime_shadow_instances(self.opaque_instances) > 0);
        self.stats.shadow_instances = if shadows { self.opaque_instances } else { 0 };
        let shadow_groups = if shadows {
            self.groups
                .iter()
                .filter(|group| !group.translucent)
                .count()
        } else {
            0
        };
        let draw_groups = self
            .groups
            .len()
            .checked_add(shadow_groups)
            .ok_or_else(|| limit("ligand pose draw groups", u64::from(u32::MAX)))?;
        self.stats.draw_groups = checked_count(draw_groups, "ligand pose draw groups")?;
        Ok(())
    }

    fn append_class(&mut self, shape: u32, translucent: bool) -> Result<(), RenderError> {
        for plan in &self.plans {
            let topology = if shape == POSE_SPHERE {
                plan.atom_count
            } else {
                plan.bond_count
            };
            let count = if translucent {
                plan.sampled_translucent
            } else {
                plan.sampled_opaque
            };
            let before = self.groups.len();
            append_draw(
                &mut self.args_scratch,
                &mut self.groups,
                PoseDrawRange {
                    batch_index: plan.batch_index,
                    shape,
                    topology_count: topology,
                    pose_count: count,
                    translucent,
                },
            )?;
            if self.groups.len() != before {
                let instances = u64::from(self.groups[before].instances);
                if translucent {
                    self.translucent = true;
                } else {
                    self.opaque_instances = self
                        .opaque_instances
                        .checked_add(instances)
                        .ok_or_else(|| limit("ligand pose opaque instances", u64::MAX))?;
                }
            }
        }
        Ok(())
    }

    fn prepare_buffers(&mut self, device: &D) -> Result<bool, RenderError> {
        let atom_bytes = table_bytes::<[f32; 4]>(&self.batch_scratch, |batch| batch.topology[1])?;
        let bond_bytes =
            table_bytes::<PoseBondGpu>(&self.batch_scratch, |batch| batch.topology[3])?;
        let pose_bytes = pose_table_bytes::<PoseTransformGpu>(&self.plans)?;
        let style_bytes = pose_table_bytes::<PoseStyleGpu>(&self.plans)?;
        let batch_bytes =
            checked_bytes::<PoseBatchGpu>(self.batch_scratch.len(), "ligand pose batch table")?;
        let args_bytes = checked_bytes::<DrawIndirectArgs>(
            self.args_scratch.len(),
            "ligand pose draw arguments",
        )?;
        let mut changed = ensure_buffer(
            device,
            "ligand pose atom topology",
            atom_bytes,
            BufferUsage::STORAGE,
            &mut self.atoms,
            &mut self.capacities[0],
        )?;
        changed |= ensure_buffer(
            device,
            "ligand pose bond topology",
            bond_bytes,
            BufferUsage::STORAGE,
            &mut self.bonds,
            &mut self.capacities[1],
        )?;
        changed |= ensure_buffer(
            device,
            "ligand pose transforms",
            pose_bytes,
            BufferUsage::STORAGE,
            &mut self.transforms,
            &mut self.capacities[2],
        )?;
        changed |= ensure_buffer(
            device,
            "ligand pose styles",
            style_bytes,
            BufferUsage::STORAGE,
            &mut self.styles,
            &mut self.capacities[3],
        )?;
        changed |= ensure_buffer(
            device,
            "ligand pose batches",
            batch_bytes,
            BufferUsage::STORAGE,
            &mut self.batches,
            &mut self.capacities[4],
        )?;
        changed |= ensure_buffer(
            device,
            "ligand pose indirect arguments",
            args_bytes,
            BufferUsage::INDIRECT,
            &mut self.args,
            &mut self.capacities[5],
        )?;
        Ok(changed)
    }

    pub(super) fn draws(&self) -> Option<(&D::BindGroup, &D::Buffer, &[LigandPoseDrawGroup])> {
        (!self.groups.is_empty()).then_some((
            self.group.as_ref()?,
            self.args.as_ref()?,
            self.groups.as_slice(),
        ))
    }

    pub(super) fn shadow_draws(
        &self,
        quality: bool,
    ) -> Option<(&D::BindGroup, &D::Buffer, &[LigandPoseDrawGroup])> {
        (self.opaque_instances > 0
            && (quality || realtime_shadow_instances(self.opaque_instances) > 0))
            .then(|| self.draws())?
    }

    pub(super) const fn has_translucency(&self) -> bool {
        self.translucent
    }

    pub(super) const fn statistics(&self) -> PoseTableStats {
        self.stats
    }

    fn release(&mut self) {
        self.atoms = None;
        self.bonds = None;
        self.transforms = None;
        self.styles = None;
        self.batches = None;
        self.args = None;
        self.capacities = [0; 6];
        self.group = None;
        self.groups.clear();
        self.opaque_instances = 0;
        self.translucent = false;
    }
}

fn pose_table_bytes<T>(plans: &[BatchPlan]) -> Result<u64, RenderError> {
    let rows = plans.iter().try_fold(0u64, |total, plan| {
        add_u64(
            total,
            u64::from(plan.sampled_opaque) + u64::from(plan.sampled_translucent),
            "selected ligand pose rows",
        )
    })?;
    rows.checked_mul(std::mem::size_of::<T>() as u64)
        .ok_or_else(|| limit("ligand pose table bytes", u64::MAX))
}

fn table_bytes<T>(
    batches: &[PoseBatchGpu],
    count: impl Fn(&PoseBatchGpu) -> u32,
) -> Result<u64, RenderError> {
    let rows = batches.iter().try_fold(0u64, |total, batch| {
        total
            .checked_add(u64::from(count(batch)))
            .ok_or_else(|| limit("ligand pose table rows", u64::MAX))
    })?;
    rows.checked_mul(std::mem::size_of::<T>() as u64)
        .ok_or_else(|| limit("ligand pose table bytes", u64::MAX))
}

fn ensure_buffer<D: Device>(
    device: &D,
    label: &'static str,
    needed: u64,
    usage: BufferUsage,
    buffer: &mut Option<D::Buffer>,
    capacity: &mut u64,
) -> Result<bool, RenderError> {
    let needed = needed.max(256);
    let limit = device.capabilities().max_storage_buffer_bytes;
    if needed > limit || limit == 0 {
        return Err(limit_error(label, limit));
    }
    let shrink = *capacity > needed.saturating_mul(4)
        && capacity.saturating_sub(needed) > RETAINED_BUFFER_SLACK;
    if buffer.is_some() && needed <= *capacity && !shrink {
        return Ok(false);
    }
    let grown = match needed.checked_next_power_of_two() {
        Some(value) => value,
        None => needed,
    };
    *capacity = grown.min(limit);
    *buffer = Some(device.create_buffer(&BufferDesc {
        label,
        size: *capacity,
        usage: usage.union(BufferUsage::COPY_DST),
    })?);
    Ok(true)
}

fn limit(resource: &'static str, ceiling: u64) -> RenderError {
    molgfx_gpu::GpuError::LimitExceeded {
        resource,
        limit: ceiling,
    }
    .into()
}

fn add_u64(left: u64, right: u64, resource: &'static str) -> Result<u64, RenderError> {
    left.checked_add(right)
        .ok_or_else(|| limit(resource, u64::MAX))
}

fn limit_error(resource: &'static str, ceiling: u64) -> RenderError {
    limit(resource, ceiling)
}
