//! Placement-local GPU state over shared immutable asset buffers.

use super::asset::GpuAsset;
use super::asset_arena::AssetArena;
use super::buffers::upload_grow;
use super::trajectory_slot::GpuTrajectory;
use super::uniforms::ModelUniforms;
use crate::error::RenderError;
use molgfx_core::{PlacedStructure, StructureHandle};
use molgfx_gpu::{BindGroupEntry, BufferDesc, BufferUsage, Device, Queue};
use molgfx_math::Mat4;
use std::sync::Arc;

#[derive(Debug)]
pub(super) struct GpuStructure<D: Device> {
    pub(super) handle: StructureHandle,
    asset: Arc<GpuAsset<D>>,
    pub(super) model: Option<D::Buffer>,
    trajectory_bvh_nodes: Option<D::Buffer>,
    trajectory_bvh_indices: Option<D::Buffer>,
    bvh_indices_upload: Vec<u32>,
    bvh_nodes_capacity: u64,
    bvh_indices_capacity: u64,
    model_to_world: Option<[f32; 16]>,
    model_needs_settle: bool,
    pick_pages: [u32; 9],
    pub(super) binding_revision: u64,
    bvh_source: BvhSource,
    trajectory: GpuTrajectory<D>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum BvhSource {
    Asset,
    Trajectory(u64),
}

impl<D: Device> GpuStructure<D> {
    pub(super) fn new(handle: StructureHandle, asset: Arc<GpuAsset<D>>) -> Self {
        Self {
            handle,
            asset,
            model: None,
            trajectory_bvh_nodes: None,
            trajectory_bvh_indices: None,
            bvh_indices_upload: Vec::new(),
            bvh_nodes_capacity: 0,
            bvh_indices_capacity: 0,
            model_to_world: None,
            model_needs_settle: false,
            pick_pages: [u32::MAX; 9],
            binding_revision: 1,
            bvh_source: BvhSource::Asset,
            trajectory: GpuTrajectory::new(),
        }
    }

    pub(super) fn replace_asset(&mut self, asset: Arc<GpuAsset<D>>) {
        if !Arc::ptr_eq(&self.asset, &asset) {
            self.asset = asset;
            self.bvh_source = BvhSource::Asset;
            self.binding_revision = self.binding_revision.wrapping_add(1);
        }
    }

    pub(super) fn set_pick_pages(&mut self, pick_pages: [u32; 9]) -> bool {
        if self.pick_pages == pick_pages {
            return false;
        }
        self.pick_pages = pick_pages;
        self.model_to_world = None;
        true
    }

    pub(super) fn pick_page(&self, kind: molgfx_core::EntityKind) -> u32 {
        self.pick_pages[super::picking_pages::kind_index(kind)]
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
        let trajectory =
            self.trajectory
                .sync(device, queue, trajectory_layout, placed.trajectory())?;
        if trajectory.coordinate_binding_changed {
            self.binding_revision = self.binding_revision.wrapping_add(1);
        }
        changed |= trajectory.changed;
        if requires_bvh && placed.trajectory().is_some() {
            let source = BvhSource::Trajectory(placed.trajectory_pair_revision());
            if self.bvh_source != source {
                self.sync_trajectory_bvh(device, queue, placed)?;
                self.bvh_source = source;
                changed = true;
            }
        } else if self.bvh_source != BvhSource::Asset {
            self.trajectory_bvh_nodes = None;
            self.trajectory_bvh_indices = None;
            self.bvh_nodes_capacity = 0;
            self.bvh_indices_capacity = 0;
            self.bvh_source = BvhSource::Asset;
            self.binding_revision = self.binding_revision.wrapping_add(1);
            changed = true;
        }
        Ok(self.sync_model(device, queue, placed)? || changed)
    }

    fn sync_trajectory_bvh(
        &mut self,
        device: &D,
        queue: &D::Queue,
        placed: &PlacedStructure,
    ) -> Result<(), RenderError> {
        let bvh = placed.render_bvh()?;
        self.bvh_indices_upload.clear();
        self.bvh_indices_upload
            .extend_from_slice(&bvh.primitive_indices);
        self.bvh_indices_upload.extend_from_slice(&bvh.escape);
        upload_grow(
            device,
            queue,
            "placement trajectory BVH nodes",
            &bvh.nodes,
            &mut self.trajectory_bvh_nodes,
            &mut self.bvh_nodes_capacity,
        )?;
        upload_grow(
            device,
            queue,
            "placement trajectory BVH indices",
            &self.bvh_indices_upload,
            &mut self.trajectory_bvh_indices,
            &mut self.bvh_indices_capacity,
        )?;
        self.binding_revision = self.binding_revision.wrapping_add(1);
        Ok(())
    }

    pub(super) fn coords_entry<'a>(
        &'a self,
        arena: &'a AssetArena<D>,
        binding: u32,
    ) -> BindGroupEntry<'a, D> {
        match self.trajectory.output() {
            Some(output) => BindGroupEntry::Buffer {
                binding,
                buffer: output,
            },
            None => arena.entry(binding, self.asset.coordinates()),
        }
    }

    pub(super) fn model_entry(&self, binding: u32) -> Option<BindGroupEntry<'_, D>> {
        self.model
            .as_ref()
            .map(|buffer| BindGroupEntry::Buffer { binding, buffer })
    }

    pub(super) fn base_coords_entry<'a>(
        &'a self,
        arena: &'a AssetArena<D>,
        binding: u32,
    ) -> BindGroupEntry<'a, D> {
        arena.entry(binding, self.asset.coordinates())
    }

    pub(super) fn previous_coords_entry<'a>(
        &'a self,
        arena: &'a AssetArena<D>,
        binding: u32,
    ) -> BindGroupEntry<'a, D> {
        match self.trajectory.previous_output() {
            Some(output) => BindGroupEntry::Buffer {
                binding,
                buffer: output,
            },
            None => arena.entry(binding, self.asset.coordinates()),
        }
    }

    pub(super) fn bvh_nodes_entry<'a>(
        &'a self,
        arena: &'a AssetArena<D>,
        binding: u32,
    ) -> BindGroupEntry<'a, D> {
        match self.trajectory_bvh_nodes.as_ref() {
            Some(nodes) => BindGroupEntry::Buffer {
                binding,
                buffer: nodes,
            },
            None => arena.entry(binding, self.asset.bvh_nodes()),
        }
    }

    pub(super) fn bvh_indices_entry<'a>(
        &'a self,
        arena: &'a AssetArena<D>,
        binding: u32,
    ) -> BindGroupEntry<'a, D> {
        match self.trajectory_bvh_indices.as_ref() {
            Some(indices) => BindGroupEntry::Buffer {
                binding,
                buffer: indices,
            },
            None => arena.entry(binding, self.asset.bvh_indices()),
        }
    }

    #[cfg(test)]
    pub(super) fn test_ranges(&self) -> [super::asset_arena::AssetRange; 3] {
        [
            self.asset.coordinates(),
            self.asset.bvh_nodes(),
            self.asset.bvh_indices(),
        ]
    }

    #[cfg(test)]
    pub(super) const fn test_model(&self) -> Option<&D::Buffer> {
        self.model.as_ref()
    }

    pub(super) fn invalidate_asset_binding(&mut self) {
        self.binding_revision = self.binding_revision.wrapping_add(1);
    }

    pub(super) const fn trajectory_dirty(&self) -> bool {
        self.trajectory.dirty()
    }

    pub(super) fn record_trajectory<P: molgfx_gpu::ComputePassEncoder<D>>(
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
        let initialized = self.model_to_world.is_some();
        let transform_changed = self.model_to_world != Some(value);
        if transform_changed || self.model_needs_settle {
            let previous = match self.model_to_world {
                Some(matrix) if transform_changed => Mat4::from_cols_array(&matrix),
                _ => placed.model_to_world,
            };
            if let Some(model) = &self.model {
                queue.write_buffer(
                    model,
                    0,
                    bytemuck::bytes_of(&ModelUniforms::new(
                        placed.model_to_world,
                        previous,
                        self.pick_pages,
                    )),
                );
            }
            self.model_to_world = Some(value);
            self.model_needs_settle = transform_changed && initialized;
            changed |= transform_changed;
        }
        Ok(changed)
    }
}
