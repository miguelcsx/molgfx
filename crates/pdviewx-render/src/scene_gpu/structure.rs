//! Persistent GPU resources shared by every representation of one structure.

use super::buffers::upload_grow;
use super::trajectory_slot::GpuTrajectory;
use super::uniforms::ModelUniforms;
use crate::error::RenderError;
use pdviewx_core::{PlacedStructure, StructureHandle};
use pdviewx_gpu::{BufferDesc, BufferUsage, Device, Queue};
use pdviewx_math::Mat4;

#[derive(Debug)]
pub(super) struct GpuStructure<D: Device> {
    pub(super) handle: StructureHandle,
    base_coords: Option<D::Buffer>,
    pub(super) model: Option<D::Buffer>,
    pub(super) bvh_nodes: Option<D::Buffer>,
    pub(super) bvh_indices: Option<D::Buffer>,
    pub(super) bvh_escape: Option<D::Buffer>,
    coords_capacity: u64,
    bvh_nodes_capacity: u64,
    bvh_indices_capacity: u64,
    bvh_escape_capacity: u64,
    coordinate_generation: Option<u64>,
    model_to_world: Option<[f32; 16]>,
    model_needs_settle: bool,
    structure_id: u32,
    pub(super) binding_revision: u64,
    bvh_source: Option<BvhSource>,
    trajectory: GpuTrajectory<D>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum BvhSource {
    Parsed(u64),
    Trajectory(u64),
}

impl<D: Device> GpuStructure<D> {
    pub(super) const fn structure_id(&self) -> u32 {
        self.structure_id
    }

    pub(super) fn new(handle: StructureHandle, structure_id: u32) -> Self {
        Self {
            handle,
            base_coords: None,
            model: None,
            bvh_nodes: None,
            bvh_indices: None,
            bvh_escape: None,
            coords_capacity: 0,
            bvh_nodes_capacity: 0,
            bvh_indices_capacity: 0,
            bvh_escape_capacity: 0,
            coordinate_generation: None,
            model_to_world: None,
            model_needs_settle: false,
            structure_id,
            binding_revision: 0,
            bvh_source: None,
            trajectory: GpuTrajectory::new(),
        }
    }

    pub(super) fn set_structure_id(&mut self, structure_id: u32) {
        if self.structure_id != structure_id {
            self.structure_id = structure_id;
            self.model_to_world = None;
        }
    }

    pub(super) fn sync(
        &mut self,
        device: &D,
        queue: &D::Queue,
        placed: &PlacedStructure,
        trajectory_layout: &D::BindGroupLayout,
        requires_bvh: bool,
    ) -> Result<bool, RenderError> {
        let mut changed = false;
        let coord_bytes = placed.atoms.coords().as_bytes();
        let needed = coord_bytes.len() as u64;
        if self.base_coords.is_none() || needed > self.coords_capacity {
            let capacity = needed.next_power_of_two().max(256);
            self.base_coords = Some(device.create_buffer(&BufferDesc {
                label: "coordinate column",
                size: capacity,
                usage: BufferUsage::STORAGE.union(BufferUsage::COPY_DST),
            })?);
            self.coords_capacity = capacity;
            self.coordinate_generation = None;
            self.binding_revision = self.binding_revision.wrapping_add(1);
            changed = true;
        }
        let generation = placed.atoms.coords().generation();
        if self.coordinate_generation != Some(generation) {
            if let Some(coords) = &self.base_coords {
                queue.write_buffer(coords, 0, coord_bytes);
            }
            self.coordinate_generation = Some(generation);
            changed = true;
        }
        let trajectory =
            self.trajectory
                .sync(device, queue, trajectory_layout, placed.trajectory())?;
        if trajectory.coordinate_binding_changed {
            self.binding_revision = self.binding_revision.wrapping_add(1);
        }
        changed |= trajectory.changed;
        if requires_bvh {
            let bvh_source = match placed.trajectory() {
                Some(_) => BvhSource::Trajectory(placed.trajectory_pair_revision()),
                None => BvhSource::Parsed(generation),
            };
            if self.bvh_source != Some(bvh_source) {
                self.sync_bvh(device, queue, placed)?;
                self.bvh_source = Some(bvh_source);
                changed = true;
            }
        } else if self.bvh_nodes.is_none() {
            self.sync_bvh_fallback(device, queue)?;
            changed = true;
        }
        Ok(self.sync_model(device, queue, placed)? || changed)
    }

    fn sync_bvh(
        &mut self,
        device: &D,
        queue: &D::Queue,
        placed: &PlacedStructure,
    ) -> Result<(), RenderError> {
        let bvh = placed.render_bvh();
        let nodes_rebind = needs_growth::<pdviewx_math::BvhNode, D>(
            bvh.nodes.len(),
            self.bvh_nodes.as_ref(),
            self.bvh_nodes_capacity,
        );
        let indices_rebind = needs_growth::<u32, D>(
            bvh.primitive_indices.len(),
            self.bvh_indices.as_ref(),
            self.bvh_indices_capacity,
        );
        let escape_rebind = needs_growth::<u32, D>(
            bvh.escape.len(),
            self.bvh_escape.as_ref(),
            self.bvh_escape_capacity,
        );
        upload_grow(
            device,
            queue,
            "shared BVH nodes",
            &bvh.nodes,
            &mut self.bvh_nodes,
            &mut self.bvh_nodes_capacity,
        )?;
        upload_grow(
            device,
            queue,
            "shared BVH primitive indices",
            &bvh.primitive_indices,
            &mut self.bvh_indices,
            &mut self.bvh_indices_capacity,
        )?;
        upload_grow(
            device,
            queue,
            "shared BVH escape indices",
            &bvh.escape,
            &mut self.bvh_escape,
            &mut self.bvh_escape_capacity,
        )?;
        if nodes_rebind || indices_rebind || escape_rebind {
            self.binding_revision = self.binding_revision.wrapping_add(1);
        }
        Ok(())
    }

    fn sync_bvh_fallback(&mut self, device: &D, queue: &D::Queue) -> Result<(), RenderError> {
        let revision = self.binding_revision;
        upload_grow(
            device,
            queue,
            "shared BVH nodes",
            &[] as &[pdviewx_math::BvhNode],
            &mut self.bvh_nodes,
            &mut self.bvh_nodes_capacity,
        )?;
        upload_grow(
            device,
            queue,
            "shared BVH primitive indices",
            &[] as &[u32],
            &mut self.bvh_indices,
            &mut self.bvh_indices_capacity,
        )?;
        upload_grow(
            device,
            queue,
            "shared BVH escape indices",
            &[] as &[u32],
            &mut self.bvh_escape,
            &mut self.bvh_escape_capacity,
        )?;
        if self.bvh_nodes.is_some() && self.bvh_indices.is_some() && self.bvh_escape.is_some() {
            self.binding_revision = revision.wrapping_add(1);
        }
        Ok(())
    }

    pub(super) fn coords(&self) -> Option<&D::Buffer> {
        self.trajectory.output().or(self.base_coords.as_ref())
    }

    pub(super) fn previous_coords(&self) -> Option<&D::Buffer> {
        self.trajectory
            .previous_output()
            .or(self.base_coords.as_ref())
    }

    pub(super) const fn trajectory_dirty(&self) -> bool {
        self.trajectory.dirty()
    }

    pub(super) fn record_trajectory<P: pdviewx_gpu::ComputePassEncoder<D>>(
        &mut self,
        pass: &mut P,
        pipeline: &D::Pipeline,
    ) {
        self.trajectory.record(pass, pipeline);
    }

    fn sync_model(
        &mut self,
        device: &D,
        queue: &D::Queue,
        placed: &PlacedStructure,
    ) -> Result<bool, RenderError> {
        let mut changed = false;
        if self.model.is_none() {
            self.model = Some(device.create_buffer(&BufferDesc {
                label: "model transform",
                size: std::mem::size_of::<ModelUniforms>() as u64,
                usage: BufferUsage::UNIFORM.union(BufferUsage::COPY_DST),
            })?);
            self.model_to_world = None;
            self.binding_revision = self.binding_revision.wrapping_add(1);
            changed = true;
        }
        let value = placed.model_to_world.to_cols_array();
        let was_initialized = self.model_to_world.is_some();
        let transform_changed = self.model_to_world != Some(value);
        if transform_changed || self.model_needs_settle {
            let previous = if transform_changed {
                self.model_to_world.map_or(placed.model_to_world, |matrix| {
                    Mat4::from_cols_array(&matrix)
                })
            } else {
                placed.model_to_world
            };
            if let Some(model) = &self.model {
                queue.write_buffer(
                    model,
                    0,
                    bytemuck::bytes_of(&ModelUniforms::new(
                        placed.model_to_world,
                        previous,
                        self.structure_id,
                    )),
                );
            }
            self.model_to_world = Some(value);
            // Creation has no prior object pose and therefore needs no second
            // settling upload. A later transform edit keeps its real previous
            // pose for one frame, then converges to zero object motion.
            self.model_needs_settle = transform_changed && was_initialized;
            changed = transform_changed;
        }
        Ok(changed)
    }
}

fn needs_growth<T, D: Device>(len: usize, buffer: Option<&D::Buffer>, capacity: u64) -> bool {
    let needed = (len as u64).saturating_mul(std::mem::size_of::<T>() as u64);
    buffer.is_none() || needed > capacity
}
