//! Cache reset when one engine begins rendering a distinct scene.

use super::{FrameUniforms, GpuInteractions, GpuLabels, GpuOverlays, GpuPrimitives, GpuScene};
use pdviewx_gpu::{Device, Queue};

impl<D: Device> GpuScene<D> {
    pub fn write_frame_uniforms(&self, queue: &D::Queue, uniforms: &FrameUniforms) {
        queue.write_buffer(&self.frame_uniforms, 0, bytemuck::bytes_of(uniforms));
    }

    pub(super) fn begin_scene(&mut self, identity: u64) -> bool {
        if self.scene_identity == Some(identity) {
            return false;
        }
        self.scene_identity = Some(identity);
        self.structures.clear();
        self.slots.clear();
        self.volume_resources.clear();
        self.volume_slots.clear();
        self.mesh_slots.clear();
        self.segmentation_resources.clear();
        self.segmentation_slots.clear();
        self.interactions = GpuInteractions::new();
        self.primitive = GpuPrimitives::new();
        self.labels = GpuLabels::new();
        self.overlays = GpuOverlays::new();
        self.structure_revision = None;
        self.slot_structure_revision = None;
        self.representation_revision = None;
        self.volume_slot_revision = None;
        self.segmentation_slot_revision = None;
        self.mesh_synced = None;
        true
    }
}
