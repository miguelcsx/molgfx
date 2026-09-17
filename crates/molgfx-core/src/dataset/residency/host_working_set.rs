//! Host payload ownership layered over the residency state machine.
//!
//! The working set accepts only caller-delivered chunks. It performs no I/O,
//! owns no catalog, and stores one map entry per retained payload.

use super::{
    DeviceLossReport, FailureReason, ResidencyBudget, ResidencyError, ResidencyKey,
    ResidencyMachine, ResidencyOutput, ResidencyPhase, ResidencyRequest, ResidencyTicket,
    StaleCompletion,
};
use crate::{ChunkData, ChunkFootprint, ChunkId, DatasetId};
use std::collections::BTreeMap;
use thiserror::Error;

#[cfg(test)]
#[path = "host_working_set_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "host_working_set_payload_tests.rs"]
mod payload_tests;

/// Typed rejection from the host working-set boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum HostWorkingSetError {
    /// A delivered chunk belongs to a different dataset than its ticket.
    #[error("delivered chunk belongs to dataset {actual}, expected {expected}")]
    DatasetMismatch {
        /// Dataset encoded by the active request.
        expected: DatasetId,
        /// Dataset carried by the payload.
        actual: DatasetId,
    },
    /// A delivered chunk has a different identity than its ticket.
    #[error("delivered chunk is {actual}, expected {expected}")]
    ChunkMismatch {
        /// Chunk encoded by the active request.
        expected: ChunkId,
        /// Chunk carried by the payload.
        actual: ChunkId,
    },
    /// The request and catalog disagree on resource accounting.
    #[error("delivered chunk footprint {actual:?} differs from requested {expected:?}")]
    FootprintMismatch {
        /// Resource charges admitted by the residency machine.
        expected: ChunkFootprint,
        /// Resource charges validated with the catalog payload.
        actual: ChunkFootprint,
    },
    /// An asynchronous completion no longer belongs to the active generation.
    #[error("stale residency completion for generation {}", .0.ticket.generation())]
    StaleCompletion(StaleCompletion),
    /// The underlying lifecycle transition was invalid or exceeded a budget.
    #[error(transparent)]
    Residency(#[from] ResidencyError),
}

/// Declarative owner of CPU payloads in the active residency working set.
#[derive(Debug)]
pub struct HostWorkingSet {
    machine: ResidencyMachine,
    payloads: BTreeMap<ResidencyKey, ChunkData>,
}

impl HostWorkingSet {
    /// Creates an empty working set with independent hard budgets.
    #[must_use]
    pub fn new(budget: ResidencyBudget) -> Self {
        Self {
            machine: ResidencyMachine::new(budget),
            payloads: BTreeMap::new(),
        }
    }

    /// Read-only lifecycle and accounting state.
    #[must_use]
    pub const fn residency(&self) -> &ResidencyMachine {
        &self.machine
    }

    /// Number of payloads retained in the CPU working set.
    #[must_use]
    pub fn retained_payloads(&self) -> usize {
        self.payloads.len()
    }

    /// Returns a retained payload without copying its provider storage.
    #[must_use]
    pub fn payload(&self, key: ResidencyKey) -> Option<&ChunkData> {
        self.payloads.get(&key)
    }

    /// Starts a generation and emits its caller-owned provider request.
    ///
    /// # Errors
    ///
    /// Returns a typed budget or generation error without performing I/O.
    pub fn request_into(
        &mut self,
        request: ResidencyRequest,
        output: &mut ResidencyOutput,
    ) -> Result<ResidencyTicket, HostWorkingSetError> {
        let ticket = self.machine.request_into(request, output)?;
        self.payloads.remove(&request.key);
        self.reconcile(output);
        Ok(ticket)
    }

    /// Accepts one exact provider delivery and retains its native storage.
    ///
    /// # Errors
    ///
    /// Rejects mismatched identities, stale generations, invalid phases and
    /// payloads that cannot fit the CPU budget.
    pub fn deliver_into(
        &mut self,
        ticket: ResidencyTicket,
        payload: ChunkData,
        output: &mut ResidencyOutput,
    ) -> Result<(), HostWorkingSetError> {
        validate_identity(ticket.key, &payload)?;
        self.validate_footprint(ticket, &payload)?;
        let transition = self.machine.ready_cpu_into(ticket, output);
        self.reconcile(output);
        let accepted = transition?;
        if !accepted {
            return Err(stale_error(ticket, &self.machine, output));
        }
        self.payloads.insert(ticket.key, payload);
        self.reconcile(output);
        Ok(())
    }

    /// Moves a retained CPU payload into the uploading phase.
    ///
    /// # Errors
    ///
    /// Rejects stale generations, invalid phases and staging pressure.
    pub fn begin_upload_into(
        &mut self,
        ticket: ResidencyTicket,
        output: &mut ResidencyOutput,
    ) -> Result<(), HostWorkingSetError> {
        let transition = self.machine.begin_upload_into(ticket, output);
        self.reconcile(output);
        let accepted = transition?;
        self.require_current(ticket, accepted, output)
    }

    /// Returns a backpressured upload to CPU-ready without dropping storage.
    ///
    /// # Errors
    ///
    /// Rejects stale generations and phases other than uploading.
    pub fn defer_upload_into(
        &mut self,
        ticket: ResidencyTicket,
        output: &mut ResidencyOutput,
    ) -> Result<(), HostWorkingSetError> {
        let transition = self.machine.defer_upload_into(ticket, output);
        self.reconcile(output);
        let accepted = transition?;
        self.require_current(ticket, accepted, output)
    }

    /// Commits an upload while retaining its CPU payload for reuse.
    ///
    /// # Errors
    ///
    /// Rejects stale generations, invalid phases and device pressure.
    pub fn complete_upload_into(
        &mut self,
        ticket: ResidencyTicket,
        output: &mut ResidencyOutput,
    ) -> Result<(), HostWorkingSetError> {
        let transition = self.machine.complete_upload_into(ticket, output);
        self.reconcile(output);
        let accepted = transition?;
        self.require_current(ticket, accepted, output)
    }

    /// Cancels current work and releases any retained CPU payload.
    ///
    /// # Errors
    ///
    /// Rejects a stale cancellation with its current generation and phase.
    pub fn cancel_into(
        &mut self,
        ticket: ResidencyTicket,
        output: &mut ResidencyOutput,
    ) -> Result<(), HostWorkingSetError> {
        let accepted = self.machine.cancel_into(ticket, output);
        self.require_current(ticket, accepted, output)
    }

    /// Records a current failure and releases its retained payload.
    ///
    /// # Errors
    ///
    /// Rejects a failure completion from an obsolete generation.
    pub fn fail_into(
        &mut self,
        ticket: ResidencyTicket,
        reason: FailureReason,
        output: &mut ResidencyOutput,
    ) -> Result<(), HostWorkingSetError> {
        let accepted = self.machine.fail_into(ticket, reason, output);
        self.require_current(ticket, accepted, output)
    }

    /// Applies new budgets and drops payloads evicted by pressure.
    pub fn set_budget_into(&mut self, budget: ResidencyBudget, output: &mut ResidencyOutput) {
        self.machine.set_budget_into(budget, output);
        self.reconcile(output);
        self.payloads
            .retain(|key, _| self.machine.snapshot(*key).phase != ResidencyPhase::Absent);
    }

    /// Invalidates device resources while preserving CPU-ready payloads.
    ///
    /// # Errors
    ///
    /// Returns a generation exhaustion error without changing ownership.
    pub fn device_lost_into(
        &mut self,
        output: &mut ResidencyOutput,
    ) -> Result<DeviceLossReport, HostWorkingSetError> {
        let report = self.machine.device_lost_into(output)?;
        self.reconcile(output);
        Ok(report)
    }

    fn require_current(
        &mut self,
        ticket: ResidencyTicket,
        accepted: bool,
        output: &ResidencyOutput,
    ) -> Result<(), HostWorkingSetError> {
        self.drop_if_absent(ticket.key);
        self.reconcile(output);
        if accepted {
            Ok(())
        } else {
            Err(stale_error(ticket, &self.machine, output))
        }
    }

    fn validate_footprint(
        &self,
        ticket: ResidencyTicket,
        payload: &ChunkData,
    ) -> Result<(), HostWorkingSetError> {
        let snapshot = self.machine.snapshot(ticket.key);
        if snapshot.generation != ticket.generation() {
            return Ok(());
        }
        let Some(expected) = self.machine.footprint(ticket.key) else {
            return Ok(());
        };
        let actual = payload.footprint();
        if expected != actual {
            return Err(HostWorkingSetError::FootprintMismatch { expected, actual });
        }
        Ok(())
    }

    fn reconcile(&mut self, output: &ResidencyOutput) {
        for cancellation in &output.cancellations {
            self.drop_if_absent(cancellation.key);
        }
        for eviction in &output.evictions {
            self.drop_if_absent(eviction.key);
        }
    }

    fn drop_if_absent(&mut self, key: ResidencyKey) {
        if self.machine.snapshot(key).phase == ResidencyPhase::Absent {
            self.payloads.remove(&key);
        }
    }
}

fn validate_identity(key: ResidencyKey, payload: &ChunkData) -> Result<(), HostWorkingSetError> {
    if payload.dataset_id() != key.dataset {
        return Err(HostWorkingSetError::DatasetMismatch {
            expected: key.dataset,
            actual: payload.dataset_id(),
        });
    }
    if payload.chunk_id() != key.chunk {
        return Err(HostWorkingSetError::ChunkMismatch {
            expected: key.chunk,
            actual: payload.chunk_id(),
        });
    }
    Ok(())
}

fn stale_error(
    ticket: ResidencyTicket,
    machine: &ResidencyMachine,
    output: &ResidencyOutput,
) -> HostWorkingSetError {
    let stale = if let Some(stale) = output.stale.first().copied() {
        stale
    } else {
        let current = machine.snapshot(ticket.key);
        StaleCompletion {
            ticket,
            current_generation: current.generation,
            current_phase: current.phase,
        }
    };
    HostWorkingSetError::StaleCompletion(stale)
}
