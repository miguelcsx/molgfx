//! Construction of fixed residency state before frame processing.

use super::FrameUploadCommand;
use crate::error::RenderError;
use crate::{ResidencyConfig, ResidencyMachine, ResidencyTicket, ResidencyWorkspace};
use pdviewx_core::{ChunkFootprint, ChunkId};
use pdviewx_gpu::ArenaAllocation;

pub(super) struct InitialResidency {
    pub(super) workspace: ResidencyWorkspace<FrameUploadCommand>,
    pub(super) machine: ResidencyMachine,
    pub(super) frame_ticket: ResidencyTicket,
    pub(super) frame_allocation: ArenaAllocation,
    pub(super) frame_resident_bytes: u64,
}

pub(super) fn initialize(
    config: ResidencyConfig,
    frame_bytes: u64,
) -> Result<InitialResidency, RenderError> {
    let mut frame_config = config;
    frame_config.uploads = frame_upload_config(config.uploads, frame_bytes)?;
    let mut workspace =
        ResidencyWorkspace::new(frame_config).map_err(|_| RenderError::Residency {
            reason: "invalid workspace configuration",
        })?;
    let frame_allocation =
        workspace
            .arena_mut()
            .allocate(frame_bytes)
            .map_err(|_| RenderError::Residency {
                reason: "frame uniform does not fit the paged arena",
            })?;
    let frame_resident_bytes = u64::from(frame_allocation.page_count()) * config.page_size;
    let mut machine = ResidencyMachine::new(config.machine_capacity);
    let frame_ticket = machine
        .request(
            ChunkId::new(u64::MAX),
            ChunkFootprint::new(0, frame_bytes, frame_bytes, frame_resident_bytes),
        )
        .map_err(|_| RenderError::Residency {
            reason: "residency machine has no frame-uniform slot",
        })?;
    machine
        .ready_cpu(frame_ticket)
        .map_err(|_| RenderError::Residency {
            reason: "frame uniform could not enter the ready state",
        })?;
    Ok(InitialResidency {
        workspace,
        machine,
        frame_ticket,
        frame_allocation,
        frame_resident_bytes,
    })
}

fn frame_upload_config(
    config: pdviewx_gpu::UploadRingConfig,
    frame_bytes: u64,
) -> Result<pdviewx_gpu::UploadRingConfig, RenderError> {
    config.validate().map_err(|_| RenderError::Residency {
        reason: "invalid frame upload configuration",
    })?;
    let bytes = usize::try_from(frame_bytes).map_err(|_| RenderError::Residency {
        reason: "frame uniform exceeds the host address space",
    })?;
    let aligned = bytes
        .checked_add(config.alignment.saturating_sub(1))
        .map(|value| value & !config.alignment.saturating_sub(1))
        .ok_or(RenderError::Residency {
            reason: "aligned frame uniform size exceeds the host address space",
        })?;
    if aligned > config.capacity_bytes {
        return Err(RenderError::Residency {
            reason: "frame uniform exceeds the configured upload budget",
        });
    }
    Ok(pdviewx_gpu::UploadRingConfig {
        capacity_bytes: aligned,
        ticket_capacity: 1,
        epoch_budget_bytes: bytes.min(config.epoch_budget_bytes),
        in_flight_budget_bytes: bytes.min(config.in_flight_budget_bytes),
        alignment: config.alignment,
    })
}

#[cfg(test)]
#[path = "residency_init_tests.rs"]
mod tests;
