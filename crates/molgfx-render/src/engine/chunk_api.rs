//! Public orchestration for caller-provided out-of-core chunks.

use super::{
    AttributeChunkWindow, BondChunkPlacement, ChunkPlacementId, ChunkPlacementStatus,
    ChunkResidencyError, ChunkResidencyMetrics, Engine, InstanceChunkPlacement,
    InstanceChunkWindow, PointChunkPlacement, RelationChunkPlacement, ResidentGenericChunk,
    ResidentStructureChunk, ResidentTrajectoryChunk, StructureChunkPlacement,
    TrajectoryChunkWindow,
};
use molgfx_core::{
    ChunkData, DeviceLossReport, HostWorkingSet, HostWorkingSetError, PagedSpatialAnchor,
    ResidencyBudget, ResidencyOutput, ResidencyRequest, ResidencyTicket,
};
use molgfx_gpu::Device;

impl<D: Device> Engine<D> {
    /// Replaces the bounded declarative placement set without copying coordinates.
    ///
    /// # Errors
    ///
    /// Rejects invalid transforms, duplicate identities or capacity overflow.
    pub fn set_structure_chunk_placements(
        &mut self,
        placements: &[StructureChunkPlacement],
    ) -> Result<(), ChunkResidencyError> {
        self.chunk_residency.replace_placements(placements)
    }

    /// Reports whether one declared placement currently enters the indirect batch.
    #[must_use]
    pub fn structure_chunk_placement_status(&self, id: ChunkPlacementId) -> ChunkPlacementStatus {
        self.chunk_residency.placement_status(id)
    }

    /// Replaces the bounded declarative generic point placement set.
    ///
    /// # Errors
    ///
    /// Rejects invalid transforms, duplicate identities or capacity overflow.
    pub fn set_point_chunk_placements(
        &mut self,
        placements: &[PointChunkPlacement],
    ) -> Result<(), ChunkResidencyError> {
        self.chunk_residency.replace_point_placements(placements)
    }

    /// Reports whether one generic point placement enters the indirect batch.
    #[must_use]
    pub fn point_chunk_placement_status(&self, id: ChunkPlacementId) -> ChunkPlacementStatus {
        self.chunk_residency.point_placement_status(id)
    }

    /// Replaces the bounded declarative rigid-instance placement set.
    ///
    /// # Errors
    ///
    /// Rejects duplicate identities or capacity overflow.
    pub fn set_instance_chunk_placements(
        &mut self,
        placements: &[InstanceChunkPlacement],
    ) -> Result<(), ChunkResidencyError> {
        self.chunk_residency.replace_instance_placements(placements)
    }

    /// Reports whether one rigid-instance placement enters its analytic batch.
    #[must_use]
    pub fn instance_chunk_placement_status(&self, id: ChunkPlacementId) -> ChunkPlacementStatus {
        self.chunk_residency.instance_placement_status(id)
    }

    /// Replaces the bounded two-frame windows sampled by paged instances.
    ///
    /// The CPU only updates this compact table; culling and rendering sample
    /// exact resident transform pages directly on the GPU.
    ///
    /// # Errors
    ///
    /// Rejects duplicate occurrence windows or capacity overflow atomically.
    pub fn set_instance_chunk_windows(
        &mut self,
        windows: &[InstanceChunkWindow],
    ) -> Result<(), ChunkResidencyError> {
        self.chunk_residency.replace_instance_windows(windows)
    }

    /// Replaces two-frame windows for scene-independent visual columns.
    ///
    /// # Errors
    ///
    /// Rejects duplicate stable columns or capacity overflow atomically.
    pub fn set_attribute_chunk_windows(
        &mut self,
        windows: &[AttributeChunkWindow],
    ) -> Result<(), ChunkResidencyError> {
        self.chunk_residency.replace_attribute_windows(windows)
    }

    /// Replaces the bounded set of globally anchored relation occurrences.
    ///
    /// # Errors
    ///
    /// Rejects duplicate identities or capacity overflow atomically.
    pub fn set_relation_chunk_placements(
        &mut self,
        placements: &[RelationChunkPlacement],
    ) -> Result<(), ChunkResidencyError> {
        self.chunk_residency.replace_relation_placements(placements)
    }

    /// Reports whether a relation payload and all exact anchor occurrences are resident.
    #[must_use]
    pub fn relation_chunk_placement_status(&self, id: ChunkPlacementId) -> ChunkPlacementStatus {
        self.chunk_residency.relation_placement_status(id)
    }

    /// Reports whether one globally identified spatial anchor currently maps
    /// to an exact resident occurrence and chunk-local GPU row.
    #[must_use]
    pub fn paged_spatial_anchor_is_resident(&self, anchor: PagedSpatialAnchor) -> bool {
        self.chunk_residency
            .resolve_spatial_anchor(anchor)
            .is_some()
    }

    /// Replaces the bounded declarative analytic bond placement set.
    ///
    /// # Errors
    ///
    /// Rejects invalid radii/transforms, duplicate identities or capacity overflow.
    pub fn set_bond_chunk_placements(
        &mut self,
        placements: &[BondChunkPlacement],
    ) -> Result<(), ChunkResidencyError> {
        self.chunk_residency.replace_bond_placements(placements)
    }

    /// Replaces the bounded two-frame trajectory windows.
    ///
    /// Windows become active only when their exact structure and both exact
    /// provider-frame generations are resident. Updating interpolation changes
    /// only the compact window table on the CPU.
    ///
    /// # Errors
    ///
    /// Rejects duplicate structure windows or capacity overflow.
    pub fn set_trajectory_chunk_windows(
        &mut self,
        windows: &[TrajectoryChunkWindow],
    ) -> Result<(), ChunkResidencyError> {
        self.chunk_residency.replace_trajectory_windows(windows)
    }

    /// Reports whether a declared bond generation and all atom endpoints are resident.
    #[must_use]
    pub fn bond_chunk_placement_status(&self, id: ChunkPlacementId) -> ChunkPlacementStatus {
        self.chunk_residency.bond_placement_status(id)
    }

    /// CPU payloads retained by the engine's bounded working set.
    #[must_use]
    pub const fn host_working_set(&self) -> &HostWorkingSet {
        &self.host_working_set
    }

    /// Emits one caller-owned provider request.
    ///
    /// # Errors
    ///
    /// Returns typed generation or in-flight budget errors.
    pub fn request_chunk_into(
        &mut self,
        request: ResidencyRequest,
        output: &mut ResidencyOutput,
    ) -> Result<ResidencyTicket, ChunkResidencyError> {
        let ticket = self.host_working_set.request_into(request, output)?;
        self.chunk_residency.apply(output)?;
        Ok(ticket)
    }

    /// Accepts an exact provider payload without reimplementing or copying it.
    ///
    /// # Errors
    ///
    /// Rejects stale tickets, identity mismatches and CPU pressure.
    pub fn deliver_chunk_into(
        &mut self,
        ticket: ResidencyTicket,
        payload: ChunkData,
        output: &mut ResidencyOutput,
    ) -> Result<(), ChunkResidencyError> {
        self.host_working_set
            .deliver_into(ticket, payload, output)?;
        self.chunk_residency.apply(output)
    }

    /// Explicitly stages and submits one provider-backed structure, bond or
    /// topology-aligned trajectory-frame chunk.
    ///
    /// `Resident` is not published until [`Self::poll_chunk_uploads_into`]
    /// observes the backend completion signal.
    ///
    /// # Errors
    ///
    /// Returns typed host lifecycle, arena, staging, capacity or GPU errors.
    pub fn upload_chunk_into(
        &mut self,
        ticket: ResidencyTicket,
        output: &mut ResidencyOutput,
    ) -> Result<(), ChunkResidencyError> {
        self.host_working_set.begin_upload_into(ticket, output)?;
        let staged = match self.host_working_set.payload(ticket.key) {
            Some(payload) => self
                .chunk_residency
                .stage(&self.device, &self.queue, ticket, payload),
            None => Err(ChunkResidencyError::PayloadMissing),
        };
        if let Err(error) = staged {
            self.host_working_set.defer_upload_into(ticket, output)?;
            self.chunk_residency.apply(output)?;
            return Err(error);
        }
        Ok(())
    }

    /// Polls backend completion and promotes only signalled uploads.
    ///
    /// # Errors
    ///
    /// Device loss and current-generation lifecycle failures are returned.
    pub fn poll_chunk_uploads_into(
        &mut self,
        output: &mut ResidencyOutput,
    ) -> Result<(), ChunkResidencyError> {
        self.chunk_residency.poll(&self.device, &self.queue)?;
        let completed = self.chunk_residency.completed().len();
        for index in 0..completed {
            let ticket = self.chunk_residency.completed()[index];
            match self.host_working_set.complete_upload_into(ticket, output) {
                Ok(()) => self.chunk_residency.apply(output)?,
                Err(HostWorkingSetError::StaleCompletion(_)) => {
                    self.chunk_residency.discard(ticket)?;
                }
                Err(error) => {
                    self.chunk_residency.discard(ticket)?;
                    self.chunk_residency.apply(output)?;
                    return Err(error.into());
                }
            }
        }
        Ok(())
    }

    /// Cancels one generation and retires submitted bytes only after its fence.
    ///
    /// # Errors
    ///
    /// Rejects stale cancellation tickets.
    pub fn cancel_chunk_into(
        &mut self,
        ticket: ResidencyTicket,
        output: &mut ResidencyOutput,
    ) -> Result<(), ChunkResidencyError> {
        self.host_working_set.cancel_into(ticket, output)?;
        self.chunk_residency.apply(output)
    }

    /// Applies hard budgets and releases evicted arena pages.
    ///
    /// # Errors
    ///
    /// Returns a stale physical allocation error if ownership diverged.
    pub fn set_chunk_budget_into(
        &mut self,
        budget: ResidencyBudget,
        output: &mut ResidencyOutput,
    ) -> Result<(), ChunkResidencyError> {
        self.host_working_set.set_budget_into(budget, output);
        self.chunk_residency.apply(output)
    }

    /// Invalidates GPU allocations and preserves provider storage for re-upload.
    ///
    /// # Errors
    ///
    /// Returns generation exhaustion or replacement allocation failure.
    pub fn chunk_device_lost_into(
        &mut self,
        output: &mut ResidencyOutput,
    ) -> Result<DeviceLossReport, ChunkResidencyError> {
        let report = self.host_working_set.device_lost_into(output)?;
        self.chunk_residency.reset(&self.device)?;
        Ok(report)
    }

    /// Resolves one global ticket to its chunk-local GPU range.
    #[must_use]
    pub fn resident_structure_chunk(
        &self,
        ticket: ResidencyTicket,
    ) -> Option<ResidentStructureChunk> {
        self.chunk_residency.resident(ticket)
    }

    /// Resolves one exact provider-frame ticket to its bounded GPU range.
    #[must_use]
    pub fn resident_trajectory_chunk(
        &self,
        ticket: ResidencyTicket,
    ) -> Option<ResidentTrajectoryChunk> {
        self.chunk_residency.resident_frame(ticket)
    }

    /// Resolves one exact generic payload generation to its shared arena range.
    #[must_use]
    pub fn resident_generic_chunk(&self, ticket: ResidencyTicket) -> Option<ResidentGenericChunk> {
        self.chunk_residency.resident_generic(ticket)
    }

    /// Real bounded-storage counters for chunk residency.
    #[must_use]
    pub fn chunk_residency_metrics(&self) -> ChunkResidencyMetrics {
        self.chunk_residency.metrics()
    }
}
