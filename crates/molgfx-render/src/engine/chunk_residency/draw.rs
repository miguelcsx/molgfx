//! Placement planning and scene synchronization for resident provider chunks.

use super::{ChunkGpuResidency, TrackedChunk};
use crate::engine::bond_draw_plan::ResidentAtomPage;
use crate::engine::chunk_draw_plan::{
    ResidentChunkPlacement, ResidentChunkRange, ResidentTrajectoryWindow,
};
use crate::engine::chunk_residency_support::local_offset;
use crate::engine::{ChunkPlacementId, ChunkPlacementStatus, ChunkResidencyError};
use molgfx_core::ResidencyTicket;
use molgfx_gpu::Device;

impl<D: Device> ChunkGpuResidency<D> {
    pub(in crate::engine) fn bond_placement_status(
        &self,
        id: ChunkPlacementId,
    ) -> ChunkPlacementStatus {
        self.bonds.status(id)
    }

    pub(in crate::engine) fn placement_status(&self, id: ChunkPlacementId) -> ChunkPlacementStatus {
        let Some(&index) = self.placement_index.get(&id) else {
            return ChunkPlacementStatus::Missing;
        };
        let placement = &self.placements[index];
        if self.resident_range(placement.ticket).is_some() {
            ChunkPlacementStatus::Resident
        } else {
            ChunkPlacementStatus::NotResident
        }
    }

    pub(in crate::engine) fn point_placement_status(
        &self,
        id: ChunkPlacementId,
    ) -> ChunkPlacementStatus {
        let Some(&index) = self.point_placement_index.get(&id) else {
            return ChunkPlacementStatus::Missing;
        };
        if self
            .resident_point_range(self.point_placements[index].ticket())
            .is_some()
        {
            ChunkPlacementStatus::Resident
        } else {
            ChunkPlacementStatus::NotResident
        }
    }

    pub(in crate::engine) fn instance_placement_status(
        &self,
        id: ChunkPlacementId,
    ) -> ChunkPlacementStatus {
        let Some(&index) = self.instance_placement_index.get(&id) else {
            return ChunkPlacementStatus::Missing;
        };
        let placement = &self.instance_placements[index];
        if self.resident_instance_plan(placement).is_none() {
            return ChunkPlacementStatus::NotResident;
        }
        if let Ok(window) = self
            .instance_windows
            .binary_search_by_key(&id, |window| window.placement)
            .map(|window| self.instance_windows[window])
            && (self.resident_instance_source(window.start).is_none()
                || self.resident_instance_source(window.end).is_none())
        {
            return ChunkPlacementStatus::NotResident;
        }
        ChunkPlacementStatus::Resident
    }

    pub(in crate::engine) fn relation_placement_status(
        &self,
        id: ChunkPlacementId,
    ) -> ChunkPlacementStatus {
        let Some(&index) = self.relation_placement_index.get(&id) else {
            return ChunkPlacementStatus::Missing;
        };
        let placement = &self.relation_placements[index];
        let Some(plan) = self.resident_relation_plan(placement) else {
            return ChunkPlacementStatus::NotResident;
        };
        if self.relation_dependencies_resident(&plan) {
            ChunkPlacementStatus::Resident
        } else {
            ChunkPlacementStatus::NotResident
        }
    }

    pub(super) fn draw_plan(
        &mut self,
    ) -> Result<(&[ResidentChunkPlacement], u64), ChunkResidencyError> {
        if self.planned_revision != self.revision {
            self.draw_plan.clear();
            self.trajectory_plan.clear();
            self.instance_plan.clear();
            self.relation_plan.clear();
            for placement in &self.placements {
                if let Some(range) = self.resident_range(placement.ticket) {
                    self.draw_plan.push(ResidentChunkPlacement {
                        model_to_world: placement.model_to_world,
                        representation: placement.representation,
                        entity_kind: molgfx_core::EntityKind::Atom,
                        range,
                    });
                }
            }
            if self
                .draw_plan
                .len()
                .saturating_add(self.point_placements.len())
                > self.draw_plan.capacity()
            {
                return Err(ChunkResidencyError::TrackingCapacity);
            }
            for placement in &self.point_placements {
                if let Some(range) = self.resident_point_range(placement.ticket()) {
                    self.draw_plan.push(ResidentChunkPlacement {
                        model_to_world: placement.model_to_world(),
                        representation: placement.representation(),
                        entity_kind: molgfx_core::EntityKind::Point,
                        range,
                    });
                }
            }
            for placement in &self.instance_placements {
                if let Some(mut plan) = self.resident_instance_plan(placement) {
                    if let Ok(index) = self
                        .instance_windows
                        .binary_search_by_key(&placement.id(), |window| window.placement)
                    {
                        let window = self.instance_windows[index];
                        let Some((start_byte_offset, start_rows)) =
                            self.resident_instance_source(window.start)
                        else {
                            continue;
                        };
                        let Some((end_byte_offset, end_rows)) =
                            self.resident_instance_source(window.end)
                        else {
                            continue;
                        };
                        if plan.span.row_count() != start_rows || start_rows != end_rows {
                            return Err(ChunkResidencyError::InstanceTimelineMismatch);
                        }
                        plan.timeline = Some(
                            crate::engine::chunk_draw_plan::ResidentInstanceChunkWindow {
                                start_byte_offset,
                                end_byte_offset,
                                interpolation: window.interpolation,
                            },
                        );
                    }
                    self.instance_plan.push(plan);
                }
            }
            for index in 0..self.relation_placements.len() {
                let placement = &self.relation_placements[index];
                let Some(plan) = self.resident_relation_plan(placement) else {
                    continue;
                };
                if self.relation_dependencies_resident(&plan) {
                    self.relation_plan.push(plan);
                }
            }
            for window in &self.trajectory_windows {
                let Some(structure) = self.resident_range(window.structure) else {
                    continue;
                };
                let Some(start) = self.resident_frame(window.start) else {
                    continue;
                };
                let Some(end) = self.resident_frame(window.end) else {
                    continue;
                };
                if structure.span.row_count() != start.local_rows
                    || start.local_rows != end.local_rows
                {
                    return Err(ChunkResidencyError::TrajectoryTopologyMismatch);
                }
                self.trajectory_plan.push(ResidentTrajectoryWindow {
                    structure,
                    start,
                    end,
                    interpolation: window.interpolation,
                });
            }
            self.planned_revision = self.revision;
        }
        Ok((&self.draw_plan, self.revision))
    }

    pub(in crate::engine) fn sync_scene(
        &mut self,
        scene: &mut crate::scene_gpu::GpuScene<D>,
        device: &D,
        queue: &D::Queue,
        derived_cache: &mut crate::DerivedCache,
        frame: u64,
    ) -> Result<(), crate::RenderError> {
        if self.planned_revision != self.revision {
            let _ = self.draw_plan()?;
        }
        scene.sync_paged_chunks(crate::scene_gpu::paged_chunks::PagedChunkSceneSync {
            device,
            queue,
            display_coordinates: &self.display_buffer,
            frame_coordinates: &self.frame_buffer,
            clusters: &self.cluster_buffer,
            plan: &self.draw_plan,
            trajectory: &self.trajectory_plan,
            revision: self.revision,
            binding_revision: self.binding_revision,
        })?;
        scene.sync_paged_instances(crate::scene_gpu::PagedInstancesSync {
            device,
            queue,
            source: &self.buffer,
            source_revision: self.binding_revision,
            plans: &self.instance_plan,
            derived_cache,
            frame,
        })?;
        self.plan_attribute_materializations(derived_cache, frame)?;
        scene.sync_paged_attribute_timelines(
            device,
            queue,
            &self.buffer,
            self.attribute_materialization_plans(),
            self.binding_revision,
        )?;
        let residency = &*self;
        scene.sync_paged_relations(crate::scene_gpu::PagedRelationsSync {
            device,
            queue,
            display_source: &self.display_buffer,
            generic_source: &self.buffer,
            plans: &self.relation_plan,
            instance_plans: &self.instance_plan,
            revision: self.relation_revision,
            source_revision: self.binding_revision,
            instance_timeline_revision: self.instance_timeline_revision,
            attribute_timeline_revision: self.attribute_timeline_revision,
            resolve: |anchor| residency.resolve_spatial_anchor(anchor),
            resolve_attribute: |ticket| residency.resident_visual_attribute_column(ticket),
        })?;
        self.bonds.sync_scene(
            scene,
            device,
            queue,
            &self.display_buffer,
            self.binding_revision,
        )
    }

    fn relation_dependencies_resident(
        &self,
        plan: &crate::engine::chunk_draw_plan::ResidentRelationChunkPlacement,
    ) -> bool {
        let anchors_resident = plan.relations.iter().all(|relation| {
            self.resolve_spatial_anchor(relation.start).is_some()
                && self.resolve_spatial_anchor(relation.end).is_some()
        });
        anchors_resident && self.relation_visual_resident(plan)
    }

    fn relation_visual_resident(
        &self,
        plan: &crate::engine::chunk_draw_plan::ResidentRelationChunkPlacement,
    ) -> bool {
        let Some(visual) = &plan.visual else {
            return true;
        };
        let target = molgfx_core::ChunkDomainRef::new(
            plan.ticket.key.dataset,
            plan.ticket.key.chunk,
            molgfx_core::ChunkDomainKind::Relation,
        );
        visual
            .style()
            .program()
            .attributes()
            .iter()
            .zip(visual.bindings().iter())
            .all(|(reference, binding)| {
                self.resident_visual_attribute_column(binding.ticket())
                    .is_some_and(|column| {
                        column.target == target
                            && column.local_rows == plan.span.row_count()
                            && column.kind == reference.kind()
                    })
            })
    }

    pub(in crate::engine) fn resident_range(
        &self,
        ticket: ResidencyTicket,
    ) -> Option<ResidentChunkRange> {
        let index = *self.tracked_index.get(&super::ticket_key(ticket))?;
        self.tracked.get(index).and_then(|entry| match *entry {
            TrackedChunk::Resident {
                ticket: owner,
                allocation,
                cluster_allocation,
                cluster_count,
                radius_base,
                max_radius,
                span,
                ..
            } if owner == ticket => Some(ResidentChunkRange {
                ticket,
                coordinate_allocation: allocation,
                cluster_allocation,
                cluster_count,
                radius_base,
                max_radius,
                span,
            }),
            TrackedChunk::Uploading { .. } | TrackedChunk::Resident { .. } => None,
        })
    }

    pub(super) fn rebuild_atom_pages(&mut self) -> Result<(), ChunkResidencyError> {
        self.atom_page_scratch.clear();
        for entry in &self.tracked {
            let TrackedChunk::Resident {
                ticket,
                allocation,
                span,
                ..
            } = *entry
            else {
                continue;
            };
            if self.atom_page_scratch.len() == self.atom_page_scratch.capacity() {
                return Err(ChunkResidencyError::TrackingCapacity);
            }
            self.atom_page_scratch.push(ResidentAtomPage {
                ticket,
                span,
                coordinate_base: local_offset::<f32>(allocation.byte_offset())?,
            });
        }
        Ok(())
    }
}
