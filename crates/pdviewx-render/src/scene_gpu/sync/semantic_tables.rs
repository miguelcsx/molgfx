//! Synchronization for non-molecular semantic draw tables.

use super::GpuScene;
use crate::error::RenderError;
use crate::scene_gpu::primitive_table::PrimitiveLayouts;
use pdviewx_core::Scene;
use pdviewx_gpu::Device;

impl<D: Device> GpuScene<D> {
    pub(super) fn sync_semantic_tables(
        &mut self,
        device: &D,
        queue: &D::Queue,
        scene: &Scene,
    ) -> Result<bool, RenderError> {
        let mut changed = self.interactions.sync(
            device,
            queue,
            &self.interaction_layout,
            scene,
            &self.structures,
        )?;
        changed |= self.primitive.sync(
            device,
            queue,
            &PrimitiveLayouts {
                table: &self.primitive_layout,
                motion: &self.primitive_motion_layout,
                shadow: &self.primitive_shadow_layout,
            },
            scene,
            &self.structures,
        )?;
        changed |= self.labels.sync(
            device,
            queue,
            &self.label_declutter_layout,
            &self.label_render_layout,
            scene,
            &self.structures,
        )?;
        changed |= self
            .overlays
            .sync(device, queue, &self.overlay_layout, scene)?;
        Ok(changed)
    }
}
