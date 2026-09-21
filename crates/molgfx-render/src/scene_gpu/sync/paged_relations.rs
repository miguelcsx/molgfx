//! Synchronization of resident globally anchored relation chunks.

use super::GpuScene;
use crate::engine::chunk_draw_plan::{ResidentRelationChunkPlacement, ResidentSpatialAnchor};
use crate::error::RenderError;
use crate::scene_gpu::generic_visual::GenericVisualResources;
use crate::scene_gpu::interaction_table::PagedRelationSync;
use molgfx_core::{EntityKind, PagedSpatialAnchor};
use molgfx_gpu::Device;

pub(crate) struct PagedRelationsSync<'a, D: Device, F, A> {
    pub(crate) device: &'a D,
    pub(crate) queue: &'a D::Queue,
    pub(crate) display_source: &'a D::Buffer,
    pub(crate) generic_source: &'a D::Buffer,
    pub(crate) plans: &'a [ResidentRelationChunkPlacement],
    pub(crate) instance_plans:
        &'a [crate::engine::chunk_draw_plan::ResidentInstanceChunkPlacement],
    pub(crate) revision: u64,
    pub(crate) source_revision: u64,
    pub(crate) instance_timeline_revision: u64,
    pub(crate) attribute_timeline_revision: u64,
    pub(crate) resolve: F,
    pub(crate) resolve_attribute: A,
}

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

    pub(crate) fn sync_paged_relations<F, A>(
        &mut self,
        input: PagedRelationsSync<'_, D, F, A>,
    ) -> Result<(), RenderError>
    where
        F: FnMut(PagedSpatialAnchor) -> Option<ResidentSpatialAnchor>,
        A: FnMut(
            molgfx_core::ResidencyTicket,
        ) -> Option<crate::engine::chunk_draw_plan::ResidentAttributeColumn>,
    {
        let PagedRelationsSync {
            device,
            queue,
            display_source,
            generic_source,
            plans,
            instance_plans,
            revision,
            source_revision,
            instance_timeline_revision,
            attribute_timeline_revision,
            resolve,
            resolve_attribute,
        } = input;
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
        self.interactions.sync_paged(PagedRelationSync {
            device,
            queue,
            render_layout: &self.interaction_layout,
            cull_layout: &self.relation_cull_layout,
            resolve_layout: &self.relation_resolve_layout,
            display_source,
            generic_source,
            instance_sources: &self.instance_batches,
            plans,
            instance_plans,
            picking: &self.picking_pages,
            resources,
            revision,
            source_revision,
            instance_binding_revision,
            instance_timeline_revision,
            attribute_timeline_revision,
            visual_revision: self.paged_visual_time_revision,
            resolve,
            resolve_attribute,
        })?;
        Ok(())
    }
}
