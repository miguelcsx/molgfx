//! Synchronization of resident chunk-backed analytic instances.

use super::GpuScene;
use crate::error::RenderError;
use crate::scene_gpu::generic_visual::GenericVisualResources;
use crate::scene_gpu::instance_batch_table::PagedInstanceSync;
use molgfx_gpu::Device;

pub(crate) struct PagedInstancesSync<'a, D: Device> {
    pub(crate) device: &'a D,
    pub(crate) queue: &'a D::Queue,
    pub(crate) source: &'a D::Buffer,
    pub(crate) source_revision: u64,
    pub(crate) plans: &'a [crate::engine::chunk_draw_plan::ResidentInstanceChunkPlacement],
    pub(crate) derived_cache: &'a mut crate::DerivedCache,
    pub(crate) frame: u64,
}

impl<D: Device> GpuScene<D> {
    pub(crate) fn sync_paged_instances(
        &mut self,
        input: PagedInstancesSync<'_, D>,
    ) -> Result<(), RenderError> {
        let PagedInstancesSync {
            device,
            queue,
            source,
            source_revision,
            plans,
            derived_cache,
            frame,
        } = input;
        self.paged_instance_pick_scratch.clear();
        for plan in plans {
            if self.paged_instance_pick_scratch.len() == self.paged_instance_pick_scratch.capacity()
            {
                return Err(molgfx_core::PickingError::WorkingSetFull.into());
            }
            let parts = u32::try_from(plan.template.part_count())
                .map_err(|_| molgfx_core::PickingError::CapacityTooLarge)?;
            let occurrences = plan
                .span
                .row_count()
                .checked_mul(parts)
                .ok_or(molgfx_core::PickingError::CapacityTooLarge)?;
            self.paged_instance_pick_scratch.push((
                plan.ticket.key.dataset,
                plan.ticket.key.chunk,
                molgfx_core::ChunkSpan::new(molgfx_core::LogicalRow::new(0), occurrences)?,
                molgfx_core::EntityKind::TemplatePart,
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
        self.instance_batches.sync_paged(PagedInstanceSync {
            device,
            queue,
            cull_layout: &self.generic_instance_cull_layout,
            render_layout: &self.generic_instance_render_layout,
            timeline_layout: &self.instance_timeline_layout,
            source,
            source_revision,
            picking: &self.picking_pages,
            plans,
            resources,
            derived_cache,
            frame,
        })?;
        Ok(())
    }
}
