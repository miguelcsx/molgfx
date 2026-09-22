//! End-to-end provider chunk upload over bounded residency primitives.

#[path = "chunk_residency/attribute_materialization.rs"]
mod attribute_materialization;
#[path = "chunk_residency/draw.rs"]
mod draw;
#[path = "chunk_residency/generic.rs"]
mod generic;
#[path = "chunk_residency/payload.rs"]
mod payload;
#[path = "chunk_residency/placements.rs"]
mod placements;
#[path = "chunk_residency/state.rs"]
mod state;
#[path = "chunk_residency/tracked.rs"]
mod tracked;
#[path = "chunk_trajectory.rs"]
mod trajectory;
#[path = "chunk_residency/upload_batch.rs"]
mod upload_batch;

use super::chunk_draw_plan::ChunkClusterGpu;
use super::chunk_residency_support::local_offset;
use super::{ChunkResidencyError, ChunkResidencyMetrics, ResidentStructureChunk};
use molgfx_core::{
    ChunkData, ChunkPayload, ChunkSpan, Eviction, LogicalRow, ResidencyOutput, ResidencyTicket,
};
use molgfx_gpu::{Device, Queue as _};
pub(super) use state::ChunkGpuResidency;
use tracked::{TrackedChunk, ticket_key};
pub(in crate::engine) use upload_batch::{CopyTarget, stage_segments};

#[cfg(test)]
#[path = "chunk_residency/lazy_tests.rs"]
mod lazy_tests;

impl<D: Device> ChunkGpuResidency<D> {
    pub(super) fn begin_epoch(&mut self) {
        self.uploads.begin_epoch();
        self.bonds.begin_epoch();
    }

    pub(super) fn stage(
        &mut self,
        device: &D,
        queue: &D::Queue,
        ticket: ResidencyTicket,
        data: &ChunkData,
    ) -> Result<(), ChunkResidencyError> {
        match data.payload() {
            ChunkPayload::ProviderStructure(_) => self.stage_structure(device, ticket, data),
            ChunkPayload::ProviderBond(_) => {
                self.rebuild_atom_pages()?;
                self.bonds.stage(
                    device,
                    queue,
                    ticket,
                    data.payload(),
                    &self.atom_page_scratch,
                )
            }
            ChunkPayload::ProviderFrame(_) => self.stage_frame(device, queue, ticket, data),
            ChunkPayload::PointBatch(_)
            | ChunkPayload::InstanceBatch(_)
            | ChunkPayload::RelationBatch(_)
            | ChunkPayload::Attribute(_) => self.stage_generic(device, queue, ticket, data),
            _ => Err(ChunkResidencyError::UnsupportedPayload),
        }
    }

    fn stage_structure(
        &mut self,
        device: &D,
        ticket: ResidencyTicket,
        data: &ChunkData,
    ) -> Result<(), ChunkResidencyError> {
        if self.tracked.len().saturating_add(self.generic.len()) == self.config.machine_capacity {
            return Err(ChunkResidencyError::TrackingCapacity);
        }
        let ChunkPayload::ProviderStructure(structure) = data.payload() else {
            return Err(ChunkResidencyError::UnsupportedPayload);
        };
        if structure.positions().is_empty() {
            return Err(ChunkResidencyError::EmptyChunk);
        }
        let coordinate_bytes = bytemuck::cast_slice(structure.positions());
        self.radius_scratch.clear();
        self.radius_scratch.reserve(structure.positions().len());
        for local_row in 0..structure.descriptor().rows() {
            let Some(element) = structure.atoms().element(local_row) else {
                return Err(ChunkResidencyError::AtomMetadataMissing { local_row });
            };
            self.radius_scratch
                .push(molgfx_core::vdw_radius(element.atomic_number()));
        }
        self.build_clusters(structure.positions())?;
        self.ensure_source_buffer(device)?;
        self.ensure_cluster_buffer(device)?;
        let radius_bytes = bytemuck::cast_slice(self.radius_scratch.as_slice());
        let coordinate_len =
            u64::try_from(coordinate_bytes.len()).map_err(|_| ChunkResidencyError::SizeOverflow)?;
        let total_len = coordinate_bytes
            .len()
            .checked_add(radius_bytes.len())
            .ok_or(ChunkResidencyError::SizeOverflow)?;
        let allocation = self
            .arena
            .allocate(u64::try_from(total_len).map_err(|_| ChunkResidencyError::SizeOverflow)?)?;
        let cluster_bytes = bytemuck::cast_slice(self.cluster_scratch.as_slice());
        let cluster_allocation = match self.cluster_arena.allocate(
            u64::try_from(cluster_bytes.len()).map_err(|_| ChunkResidencyError::SizeOverflow)?,
        ) {
            Ok(value) => value,
            Err(error) => {
                self.arena.release(allocation)?;
                return Err(error.into());
            }
        };
        let segments = [
            (
                coordinate_bytes,
                CopyTarget::Canonical,
                allocation.byte_offset(),
            ),
            (
                radius_bytes,
                CopyTarget::Canonical,
                allocation.byte_offset() + coordinate_len,
            ),
            (
                cluster_bytes,
                CopyTarget::Cluster,
                cluster_allocation.byte_offset(),
            ),
        ];
        if let Err(error) = stage_segments(&mut self.uploads, &mut self.staged, ticket, &segments) {
            self.arena.release(allocation)?;
            self.cluster_arena.release(cluster_allocation)?;
            return Err(error);
        }
        self.insert_tracked(TrackedChunk::Uploading {
            ticket,
            allocation,
            cluster_allocation,
            cluster_count: u32::try_from(self.cluster_scratch.len())
                .map_err(|_| ChunkResidencyError::LocalAddressOverflow)?,
            span: ChunkSpan::new(
                LogicalRow::new(structure.descriptor().logical_start().get()),
                structure.descriptor().rows(),
            )?,
            fence: None,
            local_rows: structure.descriptor().rows(),
            coordinate_bytes: coordinate_len,
            radius_base: local_offset::<f32>(allocation.byte_offset() + coordinate_len)?,
            max_radius: self.radius_scratch.iter().copied().fold(0.0_f32, f32::max),
            cancelled: false,
        });
        Ok(())
    }

    pub(super) fn poll(&mut self, device: &D, queue: &D::Queue) -> Result<(), ChunkResidencyError> {
        let completed = queue.completed_fence(device)?;
        if let Some(uploads) = self.uploads.get_mut() {
            let _ = uploads.retire(completed);
        }
        self.completed.clear();
        let mut index = 0;
        while index < self.tracked.len() {
            let current = self.tracked[index];
            let TrackedChunk::Uploading {
                ticket,
                allocation,
                cluster_allocation,
                cluster_count,
                span,
                fence,
                local_rows,
                coordinate_bytes,
                radius_base,
                max_radius,
                cancelled,
                ..
            } = current
            else {
                index += 1;
                continue;
            };
            if fence.is_some_and(|value| value > completed) {
                index += 1;
                continue;
            }
            if cancelled {
                self.arena.release(allocation)?;
                self.cluster_arena.release(cluster_allocation)?;
                self.remove_tracked(index);
            } else {
                self.tracked[index] = TrackedChunk::Resident {
                    ticket,
                    allocation,
                    cluster_allocation,
                    cluster_count,
                    span,
                    local_rows,
                    coordinate_bytes,
                    radius_base,
                    max_radius,
                };
                self.completed.push(ticket);
                self.bump_revision();
                index += 1;
            }
        }
        self.rebuild_atom_pages()?;
        self.poll_frames(completed)?;
        self.poll_generic(completed)?;
        self.bonds
            .poll(device, queue, &self.atom_page_scratch, &mut self.completed)?;
        Ok(())
    }

    pub(super) fn completed(&self) -> &[ResidencyTicket] {
        &self.completed
    }

    pub(super) fn apply(&mut self, output: &ResidencyOutput) -> Result<(), ChunkResidencyError> {
        for cancellation in &output.cancellations {
            self.cancel_generic(*cancellation);
            self.bonds.cancel_or_evict(*cancellation)?;
            self.cancel_frame(*cancellation)?;
            if let Some(&index) = self.tracked_index.get(&ticket_key(*cancellation))
                && let Some(TrackedChunk::Uploading { cancelled, .. }) = self.tracked.get_mut(index)
            {
                *cancelled = true;
            }
        }
        for eviction in &output.evictions {
            self.evict_generic(eviction.key, eviction.generation)?;
            self.bonds
                .evict_generation(eviction.key, eviction.generation)?;
            self.release_eviction(*eviction)?;
            self.evict_frame(eviction.key, eviction.generation)?;
        }
        Ok(())
    }

    pub(super) fn resident(&self, ticket: ResidencyTicket) -> Option<ResidentStructureChunk> {
        let index = *self.tracked_index.get(&ticket_key(ticket))?;
        self.tracked.get(index).and_then(|entry| match *entry {
            TrackedChunk::Resident {
                ticket: owner,
                allocation,
                cluster_allocation,
                cluster_count,
                local_rows,
                coordinate_bytes,
                ..
            } if owner == ticket => Some(ResidentStructureChunk {
                ticket,
                byte_offset: allocation.byte_offset(),
                byte_len: coordinate_bytes,
                local_rows,
                cluster_offset: local_offset::<ChunkClusterGpu>(cluster_allocation.byte_offset())
                    .ok()?,
                cluster_count,
            }),
            TrackedChunk::Uploading { .. } | TrackedChunk::Resident { .. } => None,
        })
    }

    pub(super) fn metrics(&self) -> ChunkResidencyMetrics {
        ChunkResidencyMetrics {
            arena: self.arena.metrics(),
            uploads: self.uploads.metrics(),
            tracked_chunks: self.tracked.len().saturating_add(self.generic.len()),
            tracked_capacity: self.tracked.capacity(),
        }
    }

    pub(super) fn reset(&mut self, device: &D) -> Result<(), ChunkResidencyError> {
        let binding_revision = self.binding_revision;
        let bond_binding_revision = self.bonds.binding_revision();
        let replacement = Self::new(device, self.config)?;
        let placements = std::mem::take(&mut self.placements);
        let placement_index = std::mem::take(&mut self.placement_index);
        let point_placements = std::mem::take(&mut self.point_placements);
        let point_placement_index = std::mem::take(&mut self.point_placement_index);
        let instance_placements = std::mem::take(&mut self.instance_placements);
        let instance_placement_index = std::mem::take(&mut self.instance_placement_index);
        let instance_windows = std::mem::take(&mut self.instance_windows);
        let attribute_windows = std::mem::take(&mut self.attribute_windows);
        let relation_placements = std::mem::take(&mut self.relation_placements);
        let relation_placement_index = std::mem::take(&mut self.relation_placement_index);
        let retained_bond_placements = self.bonds.declared_placements().to_vec();
        let trajectory_windows = std::mem::take(&mut self.trajectory_windows);
        *self = replacement;
        self.binding_revision = binding_revision.wrapping_add(1).max(1);
        self.bonds
            .replace_binding_revision(bond_binding_revision.wrapping_add(1).max(1));
        self.placements = placements;
        self.placement_index = placement_index;
        self.point_placements = point_placements;
        self.point_placement_index = point_placement_index;
        self.instance_placements = instance_placements;
        self.instance_placement_index = instance_placement_index;
        self.instance_windows = instance_windows;
        self.attribute_windows = attribute_windows;
        self.relation_placements = relation_placements;
        self.relation_placement_index = relation_placement_index;
        self.bonds.replace_placements(&retained_bond_placements)?;
        self.trajectory_windows = trajectory_windows;
        self.bump_revision();
        Ok(())
    }

    pub(super) fn discard(&mut self, ticket: ResidencyTicket) -> Result<(), ChunkResidencyError> {
        self.bonds.cancel_or_evict(ticket)?;
        self.cancel_frame(ticket)?;
        self.discard_generic(ticket)?;
        let Some(&index) = self.tracked_index.get(&ticket_key(ticket)) else {
            return Ok(());
        };
        self.arena.release(self.tracked[index].allocation())?;
        self.cluster_arena
            .release(self.tracked[index].cluster_allocation())?;
        self.remove_tracked(index);
        self.bump_revision();
        Ok(())
    }

    fn release_eviction(&mut self, eviction: Eviction) -> Result<(), ChunkResidencyError> {
        let Some(&index) = self.tracked_index.get(&(eviction.key, eviction.generation)) else {
            return Ok(());
        };
        match &mut self.tracked[index] {
            TrackedChunk::Uploading { cancelled, .. } => *cancelled = true,
            TrackedChunk::Resident { .. } => {
                let span = match self.tracked[index] {
                    TrackedChunk::Resident { span, .. } => span,
                    TrackedChunk::Uploading { .. } => return Ok(()),
                };
                self.bonds.invalidate_atom(eviction.key.dataset, span)?;
                let allocation = self.tracked[index].allocation();
                self.arena.release(allocation)?;
                self.cluster_arena
                    .release(self.tracked[index].cluster_allocation())?;
                self.remove_tracked(index);
                self.bump_revision();
            }
        }
        Ok(())
    }

    fn build_clusters(&mut self, positions: &[[f32; 3]]) -> Result<(), ChunkResidencyError> {
        self.cluster_scratch.clear();
        for cluster in positions.chunks(64) {
            if self.cluster_scratch.len() == self.cluster_scratch.capacity() {
                return Err(ChunkResidencyError::TrackingCapacity);
            }
            self.cluster_scratch
                .push(ChunkClusterGpu::from_positions(cluster));
        }
        Ok(())
    }

    fn insert_tracked(&mut self, entry: TrackedChunk) {
        let index = self.tracked.len();
        self.tracked_index.insert(ticket_key(entry.ticket()), index);
        self.tracked.push(entry);
    }

    fn remove_tracked(&mut self, index: usize) -> Option<TrackedChunk> {
        let removed = self.tracked.get(index).copied()?;
        self.tracked_index.remove(&ticket_key(removed.ticket()));
        let removed = self.tracked.swap_remove(index);
        if let Some(moved) = self.tracked.get(index).copied() {
            self.tracked_index.insert(ticket_key(moved.ticket()), index);
        }
        Some(removed)
    }

    fn bump_revision(&mut self) {
        self.revision = self.revision.wrapping_add(1).max(1);
        self.relation_revision = self.relation_revision.wrapping_add(1).max(1);
    }

    fn bump_timeline_revision(&mut self) {
        self.revision = self.revision.wrapping_add(1).max(1);
    }
}
