//! Compact ownership records for provider-backed paged allocations.

use molgfx_core::{ChunkSpan, ResidencyKey, ResidencyTicket};
use molgfx_gpu::{ArenaAllocation, FenceValue};

#[derive(Clone, Copy, Debug)]
pub(super) enum TrackedChunk {
    Uploading {
        ticket: ResidencyTicket,
        allocation: ArenaAllocation,
        cluster_allocation: ArenaAllocation,
        cluster_count: u32,
        span: ChunkSpan,
        /// Fence of the epoch submission carrying the staged bytes; `None`
        /// until the epoch is flushed.
        fence: Option<FenceValue>,
        local_rows: u32,
        coordinate_bytes: u64,
        radius_base: u32,
        max_radius: f32,
        cancelled: bool,
    },
    Resident {
        ticket: ResidencyTicket,
        allocation: ArenaAllocation,
        cluster_allocation: ArenaAllocation,
        cluster_count: u32,
        span: ChunkSpan,
        local_rows: u32,
        coordinate_bytes: u64,
        radius_base: u32,
        max_radius: f32,
    },
}

impl TrackedChunk {
    pub(super) const fn ticket(self) -> ResidencyTicket {
        match self {
            Self::Uploading { ticket, .. } | Self::Resident { ticket, .. } => ticket,
        }
    }

    pub(super) const fn allocation(self) -> ArenaAllocation {
        match self {
            Self::Uploading { allocation, .. } | Self::Resident { allocation, .. } => allocation,
        }
    }

    pub(super) const fn cluster_allocation(self) -> ArenaAllocation {
        match self {
            Self::Uploading {
                cluster_allocation, ..
            }
            | Self::Resident {
                cluster_allocation, ..
            } => cluster_allocation,
        }
    }
}

pub(super) fn ticket_key(ticket: ResidencyTicket) -> (ResidencyKey, u64) {
    (ticket.key, ticket.generation())
}
