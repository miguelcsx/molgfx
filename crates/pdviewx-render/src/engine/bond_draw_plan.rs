//! Compact plans and endpoint references for paged provider bonds.

use super::BondChunkPlacement;
use pdviewx_core::{ChunkId, ChunkSpan, DatasetId, LogicalRow, ResidencyTicket};
use pdviewx_gpu::ArenaAllocation;

/// One GPU bond referencing two scalar offsets in the shared coordinate arena.
#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct PagedBondGpu {
    pub(crate) coordinate_a: u32,
    pub(crate) coordinate_b: u32,
    pub(crate) _padding: [u32; 2],
}

/// One currently resident atom page available for global endpoint resolution.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ResidentAtomPage {
    pub(crate) ticket: ResidencyTicket,
    pub(crate) span: ChunkSpan,
    pub(crate) coordinate_base: u32,
}

impl ResidentAtomPage {
    pub(crate) fn resolve(self, dataset: DatasetId, row: LogicalRow) -> Option<u32> {
        if self.ticket.key.dataset != dataset {
            return None;
        }
        let local = self.span.local_row(self.ticket.key.chunk, row).ok()?.get();
        self.coordinate_base.checked_add(local.checked_mul(3)?)
    }
}

/// Physical state needed to lower one declarative bond placement.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ResidentBondRange {
    pub(crate) ticket: ResidencyTicket,
    pub(crate) allocation: ArenaAllocation,
    pub(crate) span: ChunkSpan,
}

/// One bond placement whose exact generation is physically resident.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ResidentBondPlacement {
    pub(crate) placement: BondChunkPlacement,
    pub(crate) range: ResidentBondRange,
}

impl ResidentBondPlacement {
    pub(crate) const fn dataset(self) -> DatasetId {
        self.range.ticket.key.dataset
    }

    pub(crate) const fn chunk(self) -> ChunkId {
        self.range.ticket.key.chunk
    }
}
