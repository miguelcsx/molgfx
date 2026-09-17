//! Bounded host staging for compact ligand topology and pose columns.
//!
//! Reusable partition indices preserve source order within each opacity class.
//! The physical transform/style table stores every opaque class first, then
//! every translucent class, while staging remains fixed at 4,096 records.

use super::ligand_pose_plan::BatchPlan;
use super::ligand_pose_sampling::distributed_source_index;
use super::ligand_pose_types::{PoseBondGpu, PoseStyleGpu, PoseTransformGpu};
use crate::error::RenderError;
use molgfx_core::Scene;
use molgfx_gpu::{Device, Queue};

const STAGING_ROWS: usize = 4_096;

pub(super) struct PoseTableBuffers<'a, D: Device> {
    pub(super) atoms: &'a D::Buffer,
    pub(super) bonds: &'a D::Buffer,
    pub(super) transforms: &'a D::Buffer,
    pub(super) styles: &'a D::Buffer,
}

pub(super) struct PoseTableScratch<'a> {
    pub(super) atoms: &'a mut Vec<[f32; 4]>,
    pub(super) bonds: &'a mut Vec<PoseBondGpu>,
    pub(super) transforms: &'a mut Vec<PoseTransformGpu>,
    pub(super) styles: &'a mut Vec<PoseStyleGpu>,
}

pub(super) fn upload_batch_tables<D: Device>(
    queue: &D::Queue,
    buffers: &PoseTableBuffers<'_, D>,
    scratch: PoseTableScratch<'_>,
    scene: &Scene,
    plans: &[BatchPlan],
    opaque_indices: &[u32],
    translucent_indices: &[u32],
) -> Result<(), RenderError> {
    let PoseTableScratch {
        atoms,
        bonds,
        transforms,
        styles,
    } = scratch;
    let mut atom_writer = StreamWriter::<D, [f32; 4]>::new(queue, buffers.atoms, atoms);
    let mut bond_writer = StreamWriter::<D, PoseBondGpu>::new(queue, buffers.bonds, bonds);
    let mut transform_writer =
        StreamWriter::<D, PoseTransformGpu>::new(queue, buffers.transforms, transforms);
    let mut style_writer = StreamWriter::<D, PoseStyleGpu>::new(queue, buffers.styles, styles);
    for plan in plans {
        let Some(batch) = scene.ligand_pose_batch(plan.handle) else {
            return Err(invalid_table("ligand pose batch handle"));
        };
        for center in batch.template().atom_centers() {
            atom_writer.push([center.x, center.y, center.z, 0.0]);
        }
        for (center, orientation, length) in batch.template().bond_frames() {
            bond_writer.push(PoseBondGpu::new(center, orientation, length));
        }
    }
    for translucent in [false, true] {
        let class_indices = if translucent {
            translucent_indices
        } else {
            opaque_indices
        };
        for plan in plans {
            let Some(batch) = scene.ligand_pose_batch(plan.handle) else {
                return Err(invalid_table("ligand pose batch handle"));
            };
            let (first, source_count, selected_count) = if translucent {
                (
                    plan.translucent_index_first,
                    plan.source_translucent,
                    plan.sampled_translucent,
                )
            } else {
                (
                    plan.opaque_index_first,
                    plan.source_opaque,
                    plan.sampled_opaque,
                )
            };
            let count = usize::try_from(source_count)
                .map_err(|_| invalid_table("ligand pose class count"))?;
            let Some(end) = first.checked_add(count) else {
                return Err(invalid_table("ligand pose index range"));
            };
            let Some(indices) = class_indices.get(first..end) else {
                return Err(invalid_table("ligand pose index range"));
            };
            for ordinal in 0..selected_count {
                let source = distributed_source_index(ordinal, source_count, selected_count);
                let source = usize::try_from(source)
                    .map_err(|_| invalid_table("ligand pose sampled index"))?;
                let Some(&index) = indices.get(source) else {
                    return Err(invalid_table("ligand pose sampled index"));
                };
                let index = usize::try_from(index)
                    .map_err(|_| invalid_table("ligand pose source index"))?;
                let Some(pose) = batch.poses().get(index).copied() else {
                    return Err(invalid_table("ligand pose source index"));
                };
                transform_writer.push(PoseTransformGpu::from(pose));
                style_writer.push(PoseStyleGpu::from(pose));
            }
        }
    }
    atom_writer.finish();
    bond_writer.finish();
    transform_writer.finish();
    style_writer.finish();
    Ok(())
}

fn invalid_table(resource: &'static str) -> RenderError {
    molgfx_gpu::GpuError::LimitExceeded {
        resource,
        limit: u64::from(u32::MAX),
    }
    .into()
}

struct StreamWriter<'a, D: Device, T: bytemuck::Pod> {
    queue: &'a D::Queue,
    buffer: &'a D::Buffer,
    scratch: &'a mut Vec<T>,
    written: u64,
}

impl<'a, D: Device, T: bytemuck::Pod> StreamWriter<'a, D, T> {
    fn new(queue: &'a D::Queue, buffer: &'a D::Buffer, scratch: &'a mut Vec<T>) -> Self {
        scratch.clear();
        if scratch.capacity() < STAGING_ROWS {
            scratch.reserve_exact(STAGING_ROWS - scratch.capacity());
        }
        Self {
            queue,
            buffer,
            scratch,
            written: 0,
        }
    }

    fn push(&mut self, value: T) {
        if self.scratch.len() == STAGING_ROWS {
            self.flush();
        }
        self.scratch.push(value);
    }

    fn finish(mut self) {
        self.flush();
    }

    fn flush(&mut self) {
        if self.scratch.is_empty() {
            return;
        }
        self.queue.write_buffer(
            self.buffer,
            self.written,
            bytemuck::cast_slice(self.scratch),
        );
        self.written += self.scratch.len() as u64 * std::mem::size_of::<T>() as u64;
        self.scratch.clear();
    }
}
