//! Persistent scalar-volume resources and representation slots.

use super::super::volume_slot::{GpuVolumeResource, OccupancySync, VolumeSync};
use super::super::volume_uniforms::VolumeUniforms;
use super::GpuScene;
use crate::error::RenderError;
use crate::passes::OccupancyPass;
use pdviewx_core::Scene;
use pdviewx_gpu::{CommandEncoder, ComputePassDesc, Device};

impl<D: Device> GpuScene<D> {
    pub(super) fn sync_volume_resources(
        &mut self,
        device: &D,
        queue: &D::Queue,
        scene: &Scene,
    ) -> Result<bool, RenderError> {
        let mut changed = false;
        let structures = &self.structures;
        for resource in &mut self.volume_resources {
            if let Some((stream, structure_handle, selected_rows)) =
                scene.occupancy_stream(resource.handle)
            {
                let Some(placed) = scene.structure(structure_handle) else {
                    continue;
                };
                let Some(structure) = structures
                    .iter()
                    .find(|structure| structure.handle == structure_handle)
                else {
                    continue;
                };
                let Some(revision) = scene.volume_content_revision(resource.handle) else {
                    continue;
                };
                changed |= resource.sync_occupancy(&OccupancySync {
                    device,
                    queue,
                    layout: &self.occupancy_layout,
                    stream,
                    selected_rows,
                    placed,
                    structure,
                    asset_arena: &self.asset_arena,
                    revision,
                })?;
            } else if let (Some(volume), Some(revision)) = (
                scene.volume(resource.handle),
                scene.volume_content_revision(resource.handle),
            ) {
                changed |= resource.sync_static(device, queue, volume, revision)?;
            }
        }
        Ok(changed)
    }

    pub(super) fn sync_volume_slots(
        &mut self,
        device: &D,
        queue: &D::Queue,
        scene: &Scene,
    ) -> Result<bool, RenderError> {
        let mut changed = false;
        for slot in &mut self.volume_slots {
            let Some(representation) = scene.representation(slot.representation) else {
                continue;
            };
            let Some(volume_handle) = representation.volume_handle() else {
                continue;
            };
            let Some((volume_view, volume_binding_revision)) = self
                .volume_resources
                .iter()
                .find(|resource| resource.handle == volume_handle)
                .and_then(GpuVolumeResource::binding)
            else {
                continue;
            };
            let Some(empty_space_bounds_view) = self
                .volume_resources
                .iter()
                .find(|resource| resource.handle == volume_handle)
                .and_then(GpuVolumeResource::empty_space_binding)
            else {
                continue;
            };
            let Some(representation_revision) =
                scene.representation_content_revision(slot.representation)
            else {
                continue;
            };
            let uniforms = if let Some(volume) = scene.volume(volume_handle) {
                VolumeUniforms::new(volume, representation)
            } else if let Some((stream, structure_handle, _)) =
                scene.occupancy_stream(volume_handle)
            {
                let Some(placed) = scene.structure(structure_handle) else {
                    continue;
                };
                VolumeUniforms::new_occupancy(stream, placed, representation)
            } else {
                continue;
            };
            changed |= slot.sync(&VolumeSync {
                device,
                queue,
                layout: &self.volume_layout,
                uniforms,
                representation,
                representation_revision,
                volume_handle,
                volume_view,
                empty_space_bounds_view,
                volume_binding_revision,
            })?;
        }
        Ok(changed)
    }

    pub fn record_occupancies(
        &mut self,
        encoder: &mut D::CommandEncoder,
        pipelines: &OccupancyPass<D>,
    ) {
        if !self
            .volume_resources
            .iter()
            .any(GpuVolumeResource::occupancy_dirty)
        {
            return;
        }
        let mut pass = encoder.begin_compute_pass(&ComputePassDesc {
            label: "temporal occupancy accumulation",
            timestamps: None,
        });
        for resource in &mut self.volume_resources {
            resource.record_occupancy(&mut pass, pipelines);
        }
    }
}
