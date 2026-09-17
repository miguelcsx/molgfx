//! Categorical-volume resource and representation reconciliation.

use super::super::segmentation_slot::{
    GpuSegmentationResource, GpuSegmentationSlot, SegmentationSync,
};
use super::GpuScene;
use crate::error::RenderError;
use molgfx_core::Scene;
use molgfx_gpu::Device;

impl<D: Device> GpuScene<D> {
    pub(super) fn reconcile_segmentations(&mut self, scene: &Scene) {
        let revision = (
            scene.segmentation_revision(),
            scene.representation_revision(),
        );
        if self.segmentation_slot_revision == Some(revision) {
            return;
        }
        self.representation_scratch.clear();
        self.representation_scratch.extend(
            scene
                .representations()
                .filter(|(_, representation)| {
                    representation.visible && representation.segmentation_handle().is_some()
                })
                .map(|(handle, representation)| (representation.order, handle)),
        );
        self.representation_scratch.sort_unstable();
        self.segmentation_handle_scratch.clear();
        self.segmentation_handle_scratch.extend(
            scene
                .representations()
                .filter(|(_, representation)| representation.visible)
                .filter_map(|(_, representation)| representation.segmentation_handle()),
        );
        self.segmentation_handle_scratch.sort_unstable();
        self.segmentation_handle_scratch.dedup();

        let mut old_resources = std::mem::take(&mut self.segmentation_resources);
        for (source_id, handle) in self.segmentation_handle_scratch.iter().enumerate() {
            let source_id = u32::try_from(source_id).map_or(u32::MAX, |value| value);
            if let Some(index) = old_resources
                .iter()
                .position(|resource| resource.handle == *handle)
            {
                let mut resource = old_resources.swap_remove(index);
                resource.set_source_id(source_id);
                self.segmentation_resources.push(resource);
            } else {
                let mut resource = GpuSegmentationResource::new(*handle);
                resource.set_source_id(source_id);
                self.segmentation_resources.push(resource);
            }
        }

        let mut old = std::mem::take(&mut self.segmentation_slots);
        for (_, handle) in &self.representation_scratch {
            if let Some(index) = old.iter().position(|slot| slot.representation == *handle) {
                self.segmentation_slots.push(old.swap_remove(index));
            } else {
                self.segmentation_slots
                    .push(GpuSegmentationSlot::new(*handle));
            }
        }
        self.segmentation_slot_revision = Some(revision);
    }

    pub(super) fn sync_segmentation_resources(
        &mut self,
        device: &D,
        queue: &D::Queue,
        scene: &Scene,
    ) -> Result<bool, RenderError> {
        let mut changed = false;
        for resource in &mut self.segmentation_resources {
            let Some(volume) = scene.segmented_volume(resource.handle) else {
                continue;
            };
            let Some(revision) = scene.segmentation_content_revision(resource.handle) else {
                continue;
            };
            changed |= resource.sync(device, queue, volume, revision)?;
        }
        Ok(changed)
    }

    pub(super) fn sync_segmentation_slots(
        &mut self,
        device: &D,
        queue: &D::Queue,
        scene: &Scene,
    ) -> Result<bool, RenderError> {
        let mut changed = false;
        for slot in &mut self.segmentation_slots {
            let Some(representation) = scene.representation(slot.representation) else {
                continue;
            };
            let Some(segmentation_handle) = representation.segmentation_handle() else {
                continue;
            };
            let Some(volume) = scene.segmented_volume(segmentation_handle) else {
                continue;
            };
            let Some(resource) = self
                .segmentation_resources
                .iter()
                .find(|resource| resource.handle == segmentation_handle)
            else {
                continue;
            };
            let Some((volume_view, volume_binding_revision)) = resource.binding() else {
                continue;
            };
            let Some(representation_revision) =
                scene.representation_content_revision(slot.representation)
            else {
                continue;
            };
            changed |= slot.sync(&SegmentationSync {
                device,
                queue,
                layout: &self.segmentation_layout,
                volume,
                representation,
                representation_revision,
                segmentation_handle,
                volume_view,
                volume_binding_revision,
                source_id: resource.source_id(),
            })?;
        }
        Ok(changed)
    }

    pub(crate) fn resolve_segment(
        &self,
        source_id: u32,
        label: u32,
    ) -> Option<molgfx_core::VolumeSegmentRef> {
        let resource = self
            .segmentation_resources
            .iter()
            .find(|resource| resource.source_id() == source_id)?;
        Some(molgfx_core::VolumeSegmentRef {
            volume: resource.handle,
            label,
        })
    }

    pub(crate) fn has_segmentation_translucency(&self) -> bool {
        self.segmentation_slots
            .iter()
            .any(GpuSegmentationSlot::source_is_drawable)
    }
}
