//! Deterministic lifecycle tracking for bounded resident resources.

use molgfx_core::{ChunkFootprint, ChunkId};
use thiserror::Error;

/// Lifecycle of one resource moving toward GPU residency.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ResidencyState {
    /// No slot is associated with the resource.
    #[default]
    Absent,
    /// Residency was requested but no host payload is ready.
    Requested,
    /// Host data is ready to stage.
    ReadyCpu,
    /// An upload command is consuming staged bytes.
    Uploading,
    /// The GPU resource contains current data.
    Resident,
}

/// Generation-checked handle for one machine slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResidencyTicket {
    slot: u32,
    generation: u64,
    chunk: ChunkId,
}

/// Current machine gauges and cumulative transition counts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ResidencyMachineMetrics {
    /// Slots currently in the resident state.
    pub resident_resources: u64,
    /// GPU bytes declared by resident resource footprints.
    pub resident_payload_bytes: u64,
    /// Successful state transitions.
    pub transitions: u64,
    /// Requests rejected because every slot was occupied.
    pub capacity_stalls: u64,
}

/// Invalid lifecycle operation or exhausted fixed capacity.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum ResidencyMachineError {
    /// Every preallocated lifecycle slot is occupied.
    #[error("residency machine capacity is exhausted")]
    Capacity,
    /// The ticket is stale or the requested transition is invalid.
    #[error("invalid residency lifecycle transition")]
    InvalidTransition,
}

#[derive(Clone, Copy, Debug)]
struct Entry {
    chunk: ChunkId,
    footprint: ChunkFootprint,
    generation: u64,
    state: ResidencyState,
}

impl Entry {
    const fn vacant() -> Self {
        Self {
            chunk: ChunkId::new(0),
            footprint: ChunkFootprint::new(0, 0, 0, 0),
            generation: 0,
            state: ResidencyState::Absent,
        }
    }
}

/// Fixed-capacity state machine with stable slot-order decisions.
#[derive(Debug)]
pub struct ResidencyMachine {
    entries: Vec<Entry>,
    next_generation: u64,
    metrics: ResidencyMachineMetrics,
}

impl ResidencyMachine {
    /// Preallocates all lifecycle slots.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: vec![Entry::vacant(); capacity],
            next_generation: 1,
            metrics: ResidencyMachineMetrics::default(),
        }
    }

    /// Associates the lowest vacant slot with one chunk.
    ///
    /// # Errors
    ///
    /// Returns [`ResidencyMachineError::Capacity`] when no slot is vacant.
    pub fn request(
        &mut self,
        chunk: ChunkId,
        footprint: ChunkFootprint,
    ) -> Result<ResidencyTicket, ResidencyMachineError> {
        let Some((index, entry)) = self
            .entries
            .iter_mut()
            .enumerate()
            .find(|(_, entry)| entry.state == ResidencyState::Absent)
        else {
            self.metrics.capacity_stalls = self.metrics.capacity_stalls.saturating_add(1);
            return Err(ResidencyMachineError::Capacity);
        };
        let generation = self.next_generation;
        self.next_generation = self.next_generation.wrapping_add(1).max(1);
        *entry = Entry {
            chunk,
            footprint,
            generation,
            state: ResidencyState::Requested,
        };
        self.metrics.transitions = self.metrics.transitions.saturating_add(1);
        let slot = u32::try_from(index).map_err(|_| ResidencyMachineError::Capacity)?;
        Ok(ResidencyTicket {
            slot,
            generation,
            chunk,
        })
    }

    /// Marks the caller payload ready for staging.
    ///
    /// # Errors
    ///
    /// The ticket must currently be requested.
    pub fn ready_cpu(&mut self, ticket: ResidencyTicket) -> Result<(), ResidencyMachineError> {
        self.transition(ticket, ResidencyState::Requested, ResidencyState::ReadyCpu)
    }

    /// Marks staging as consumed by an upload command.
    ///
    /// # Errors
    ///
    /// The ticket must currently be ready on the CPU.
    pub fn uploading(&mut self, ticket: ResidencyTicket) -> Result<(), ResidencyMachineError> {
        self.transition(ticket, ResidencyState::ReadyCpu, ResidencyState::Uploading)
    }

    /// Marks the completed upload GPU-resident.
    ///
    /// # Errors
    ///
    /// The ticket must currently be uploading.
    pub fn resident(&mut self, ticket: ResidencyTicket) -> Result<(), ResidencyMachineError> {
        self.transition(ticket, ResidencyState::Uploading, ResidencyState::Resident)?;
        let resident_bytes = self.entry(ticket)?.footprint.gpu_bytes;
        self.metrics.resident_resources = self.metrics.resident_resources.saturating_add(1);
        self.metrics.resident_payload_bytes = self
            .metrics
            .resident_payload_bytes
            .saturating_add(resident_bytes);
        Ok(())
    }

    /// Current state of a live ticket.
    #[must_use]
    pub fn state(&self, ticket: ResidencyTicket) -> Option<ResidencyState> {
        self.entry(ticket).ok().map(|entry| entry.state)
    }

    /// Current gauges and cumulative counters.
    #[must_use]
    pub const fn metrics(&self) -> ResidencyMachineMetrics {
        self.metrics
    }

    fn transition(
        &mut self,
        ticket: ResidencyTicket,
        from: ResidencyState,
        to: ResidencyState,
    ) -> Result<(), ResidencyMachineError> {
        let entry = self.entry_mut(ticket)?;
        if entry.state != from {
            return Err(ResidencyMachineError::InvalidTransition);
        }
        entry.state = to;
        self.metrics.transitions = self.metrics.transitions.saturating_add(1);
        Ok(())
    }

    fn entry(&self, ticket: ResidencyTicket) -> Result<&Entry, ResidencyMachineError> {
        let Some(entry) = self.entries.get(ticket.slot as usize) else {
            return Err(ResidencyMachineError::InvalidTransition);
        };
        if entry.generation != ticket.generation || entry.chunk != ticket.chunk {
            return Err(ResidencyMachineError::InvalidTransition);
        }
        Ok(entry)
    }

    fn entry_mut(&mut self, ticket: ResidencyTicket) -> Result<&mut Entry, ResidencyMachineError> {
        let Some(entry) = self.entries.get_mut(ticket.slot as usize) else {
            return Err(ResidencyMachineError::InvalidTransition);
        };
        if entry.generation != ticket.generation || entry.chunk != ticket.chunk {
            return Err(ResidencyMachineError::InvalidTransition);
        }
        Ok(entry)
    }
}

#[cfg(test)]
#[path = "residency_machine_tests.rs"]
mod tests;
