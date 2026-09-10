//! Synchronization of resident chunk-backed analytic instances.

use super::GpuScene;
use crate::error::RenderError;
use crate::scene_gpu::generic_visual::GenericVisualResources;
use pdviewx_gpu::Device;

impl<D: Device> GpuScene<D> {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn sync_paged_instances(
        &mut self,
        device: &D,
        queue: &D::Queue,
        source: &D::Buffer,
        source_revision: u64,
        plans: &[crate::engine::chunk_draw_plan::ResidentInstanceChunkPlacement],
        derived_cache: &mut crate::DerivedCache,
        frame: u64,
    ) -> Result<(), RenderError> {
        self.paged_instance_pick_scratch.clear();
        for plan in plans {
            if self.paged_instance_pick_scratch.len() == self.paged_instance_pick_scratch.capacity()
            {
                return Err(pdviewx_core::PickingError::WorkingSetFull.into());
            }
            let parts = u32::try_from(plan.template.part_count())
                .map_err(|_| pdviewx_core::PickingError::CapacityTooLarge)?;
            let occurrences = plan
                .span
                .row_count()
                .checked_mul(parts)
                .ok_or(pdviewx_core::PickingError::CapacityTooLarge)?;
            self.paged_instance_pick_scratch.push((
                plan.ticket.key.dataset,
                plan.ticket.key.chunk,
                pdviewx_core::ChunkSpan::new(pdviewx_core::LogicalRow::new(0), occurrences)?,
                pdviewx_core::EntityKind::TemplatePart,
            ));
        }
        self.picking_pages
            .sync_instance_chunks(&self.paged_instance_pick_scratch)?;
        let resources = GenericVisualResources {
            programs: &self.visual_programs,
            parameters: &self.visual_parameters,
            properties: &self.visual_properties,
            fallback: self.visual_fallback.entries(),
            parameter_slot_base: self.slots.len(),
            time_seconds: self.paged_visual_time_seconds,
        };
        self.instance_batches.sync_paged(
            device,
            queue,
            &self.generic_instance_cull_layout,
            &self.generic_instance_render_layout,
            &self.instance_timeline_layout,
            source,
            source_revision,
            &self.picking_pages,
            plans,
            resources,
            derived_cache,
            frame,
        )?;
        Ok(())
    }
}
