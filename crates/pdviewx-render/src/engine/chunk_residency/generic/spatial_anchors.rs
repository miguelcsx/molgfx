//! Cold-path resolution of stable spatial identities to resident GPU ranges.

use super::super::ChunkGpuResidency;
use crate::engine::chunk_draw_plan::{ResidentPositionSource, ResidentSpatialAnchor};
use pdviewx_core::{ChunkEntityRef, ChunkSpatialKind, PagedSpatialAnchor, TemplatePartChunkRef};
use pdviewx_gpu::Device;
use pdviewx_math::Vec3;

impl<D: Device> ChunkGpuResidency<D> {
    pub(in crate::engine) fn resolve_spatial_anchor(
        &self,
        anchor: PagedSpatialAnchor,
    ) -> Option<ResidentSpatialAnchor> {
        match anchor {
            PagedSpatialAnchor::World(position) => Some(ResidentSpatialAnchor::World(position)),
            PagedSpatialAnchor::Entity(reference) => self.resolve_entity_anchor(reference),
            PagedSpatialAnchor::TemplatePart(reference) => self.resolve_template_anchor(reference),
        }
    }

    fn resolve_entity_anchor(&self, reference: ChunkEntityRef) -> Option<ResidentSpatialAnchor> {
        match reference.kind() {
            ChunkSpatialKind::Atom => self.resolve_atom_anchor(reference),
            ChunkSpatialKind::Point => self.resolve_point_anchor(reference),
            ChunkSpatialKind::Instance => self.resolve_instance_anchor(reference, Vec3::ZERO),
        }
    }

    fn resolve_atom_anchor(&self, reference: ChunkEntityRef) -> Option<ResidentSpatialAnchor> {
        let index = *self.placement_index.get(&reference.occurrence())?;
        let placement = self.placements.get(index)?;
        if !same_chunk(placement.ticket, reference) {
            return None;
        }
        let range = self.resident_range(placement.ticket)?;
        let local_row = range
            .span
            .local_row(reference.chunk(), reference.row())
            .ok()?
            .get();
        Some(ResidentSpatialAnchor::Position {
            source: ResidentPositionSource::Display,
            byte_offset: range.coordinate_allocation.byte_offset(),
            byte_len: u64::from(range.span.row_count()).saturating_mul(12),
            local_row,
            model_to_world: placement.model_to_world,
        })
    }

    fn resolve_point_anchor(&self, reference: ChunkEntityRef) -> Option<ResidentSpatialAnchor> {
        let index = *self.point_placement_index.get(&reference.occurrence())?;
        let placement = self.point_placements.get(index)?;
        if !same_chunk(placement.ticket(), reference) {
            return None;
        }
        let range = self.resident_point_range(placement.ticket())?;
        let local_row = range
            .span
            .local_row(reference.chunk(), reference.row())
            .ok()?
            .get();
        Some(ResidentSpatialAnchor::Position {
            source: ResidentPositionSource::Generic,
            byte_offset: range.coordinate_allocation.byte_offset(),
            byte_len: u64::from(range.span.row_count()).saturating_mul(12),
            local_row,
            model_to_world: placement.model_to_world(),
        })
    }

    fn resolve_instance_anchor(
        &self,
        reference: ChunkEntityRef,
        local_position: Vec3,
    ) -> Option<ResidentSpatialAnchor> {
        let index = *self.instance_placement_index.get(&reference.occurrence())?;
        let placement = self.instance_placements.get(index)?;
        if !same_chunk(placement.ticket(), reference) {
            return None;
        }
        let plan = self.resident_instance_plan(placement)?;
        let local_row = plan
            .span
            .local_row(reference.chunk(), reference.row())
            .ok()?
            .get();
        let timeline = self
            .instance_windows
            .binary_search_by_key(&placement.id(), |window| window.placement)
            .ok()
            .map(|index| self.instance_windows[index]);
        let (timeline_start, timeline_end, interpolation) = if let Some(window) = timeline {
            let (start, start_rows) = self.resident_instance_source(window.start)?;
            let (end, end_rows) = self.resident_instance_source(window.end)?;
            if start_rows != plan.span.row_count() || end_rows != start_rows {
                return None;
            }
            (start, end, window.interpolation)
        } else {
            (plan.byte_offset, plan.byte_offset, 0.0)
        };
        Some(ResidentSpatialAnchor::Rigid {
            byte_offset: plan.byte_offset,
            byte_len: u64::from(plan.span.row_count()).saturating_mul(32),
            timeline_start,
            timeline_end,
            interpolation,
            local_row,
            local_position,
        })
    }

    fn resolve_template_anchor(
        &self,
        reference: TemplatePartChunkRef,
    ) -> Option<ResidentSpatialAnchor> {
        let instance = reference.instance();
        let index = *self.instance_placement_index.get(&instance.occurrence())?;
        let placement = self.instance_placements.get(index)?;
        if !same_chunk(placement.ticket(), instance) {
            return None;
        }
        let part = usize::try_from(reference.part_row()).ok()?;
        let spheres = placement.template().spheres();
        let local_position = if let Some(sphere) = spheres.get(part) {
            Vec3::from_array(sphere.center)
        } else {
            let capsule = placement
                .template()
                .capsules()
                .get(part.checked_sub(spheres.len())?)?;
            (capsule.start() + capsule.end()) * 0.5
        };
        self.resolve_instance_anchor(instance, local_position)
    }
}

fn same_chunk(ticket: pdviewx_core::ResidencyTicket, reference: ChunkEntityRef) -> bool {
    ticket.key.dataset == reference.dataset() && ticket.key.chunk == reference.chunk()
}
