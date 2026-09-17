//! Synchronization of resident globally anchored relation chunks.

use super::GpuScene;
use crate::engine::chunk_draw_plan::{ResidentRelationChunkPlacement, ResidentSpatialAnchor};
use crate::error::RenderError;
use crate::scene_gpu::generic_visual::GenericVisualResources;
use molgfx_core::{EntityKind, PagedSpatialAnchor};
use molgfx_gpu::Device;

impl<D: Device> GpuScene<D> {
    pub(crate) fn sync_paged_attribute_timelines(
        &mut self,
        device: &D,
        queue: &D::Queue,
        source: &D::Buffer,
        plans: &[crate::engine::chunk_draw_plan::ResidentAttributeMaterialization],
        source_revision: u64,
    ) -> Result<(), RenderError> {
        self.visual_properties.sync_paged_timelines(
            device,
            queue,
            &self.attribute_timeline_layout,
            source,
            source_revision,
            plans,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn sync_paged_relations<F, A>(
        &mut self,
        device: &D,
        queue: &D::Queue,
        display_source: &D::Buffer,
        generic_source: &D::Buffer,
        plans: &[ResidentRelationChunkPlacement],
        instance_plans: &[crate::engine::chunk_draw_plan::ResidentInstanceChunkPlacement],
        revision: u64,
        source_revision: u64,
        instance_timeline_revision: u64,
        attribute_timeline_revision: u64,
        resolve: F,
        resolve_attribute: A,
    ) -> Result<(), RenderError>
    where
        F: FnMut(PagedSpatialAnchor) -> Option<ResidentSpatialAnchor>,
        A: FnMut(
            molgfx_core::ResidencyTicket,
        ) -> Option<crate::engine::chunk_draw_plan::ResidentAttributeColumn>,
    {
        self.paged_relation_pick_scratch.clear();
        for plan in plans {
            if self.paged_relation_pick_scratch.len() == self.paged_relation_pick_scratch.capacity()
            {
                return Err(molgfx_core::PickingError::WorkingSetFull.into());
            }
            self.paged_relation_pick_scratch.push((
                plan.ticket.key.dataset,
                plan.ticket.key.chunk,
                plan.span,
                EntityKind::Relation,
            ));
        }
        self.picking_pages
            .sync_relation_chunks(&self.paged_relation_pick_scratch)?;
        let resources = GenericVisualResources {
            programs: &self.visual_programs,
            parameters: &self.visual_parameters,
            properties: &self.visual_properties,
            fallback: self.visual_fallback.entries(),
            parameter_slot_base: self.slots.len(),
            time_seconds: self.paged_visual_time_seconds,
        };
        let instance_binding_revision = self.instance_batches.paged_binding_revision();
        self.interactions.sync_paged(
            device,
            queue,
            &self.interaction_layout,
            &self.relation_cull_layout,
            &self.relation_resolve_layout,
            display_source,
            generic_source,
            &self.instance_batches,
            plans,
            instance_plans,
            &self.picking_pages,
            resources,
            revision,
            source_revision,
            instance_binding_revision,
            instance_timeline_revision,
            attribute_timeline_revision,
            self.paged_visual_time_revision,
            resolve,
            resolve_attribute,
        )?;
        Ok(())
    }
}
