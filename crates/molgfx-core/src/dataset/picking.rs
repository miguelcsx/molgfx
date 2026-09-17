//! Fixed-capacity translation from compact GPU picks to global dataset rows.

use crate::{ChunkId, ChunkSpan, DatasetId, EntityKind, LocalRow, LogicalRow, PickingError};

/// Global, collision-free provenance of one picked entity.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GlobalPickIdentity {
    dataset: DatasetId,
    chunk: ChunkId,
    row: LogicalRow,
    kind: EntityKind,
}

impl GlobalPickIdentity {
    /// Creates an identity from a resolved resident page and source row.
    #[must_use]
    pub const fn new(
        dataset: DatasetId,
        chunk: ChunkId,
        row: LogicalRow,
        kind: EntityKind,
    ) -> Self {
        Self {
            dataset,
            chunk,
            row,
            kind,
        }
    }

    /// Dataset containing the picked entity.
    #[must_use]
    pub const fn dataset(self) -> DatasetId {
        self.dataset
    }

    /// Chunk that supplied the resident row.
    #[must_use]
    pub const fn chunk(self) -> ChunkId {
        self.chunk
    }

    /// Stable row in the complete logical dataset.
    #[must_use]
    pub const fn row(self) -> LogicalRow {
        self.row
    }

    /// Entity namespace used to resolve cold provenance without collisions.
    #[must_use]
    pub const fn kind(self) -> EntityKind {
        self.kind
    }
}

/// Provenance shared by every local row in one resident picking page.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PickPageDescriptor {
    dataset: DatasetId,
    chunk: ChunkId,
    rows: ChunkSpan,
    kind: EntityKind,
}

impl PickPageDescriptor {
    /// Creates a page mapping for one entity namespace and chunk span.
    #[must_use]
    pub const fn new(
        dataset: DatasetId,
        chunk: ChunkId,
        rows: ChunkSpan,
        kind: EntityKind,
    ) -> Self {
        Self {
            dataset,
            chunk,
            rows,
            kind,
        }
    }

    /// Dataset containing this page.
    #[must_use]
    pub const fn dataset(self) -> DatasetId {
        self.dataset
    }

    /// Stable chunk represented by this page.
    #[must_use]
    pub const fn chunk(self) -> ChunkId {
        self.chunk
    }

    /// Logical rows represented by chunk-local indices.
    #[must_use]
    pub const fn rows(self) -> ChunkSpan {
        self.rows
    }

    /// Collision-free entity namespace for this page.
    #[must_use]
    pub const fn kind(self) -> EntityKind {
        self.kind
    }
}

/// Dense index of a page in the bounded resident working set.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct ResidentPage(u32);

impl ResidentPage {
    /// Returns the compact value written to the GPU picking attachment.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Monotonic generation of one reusable resident page slot.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct PickGeneration(u64);

impl PickGeneration {
    /// Returns the host-side generation value.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Reservation retained by upload and draw submission until completion.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PickPageTicket {
    page: ResidentPage,
    generation: PickGeneration,
}

impl PickPageTicket {
    /// Resident page encoded by this ticket.
    #[must_use]
    pub const fn page(self) -> ResidentPage {
        self.page
    }

    /// Generation that makes delayed work safe to reject.
    #[must_use]
    pub const fn generation(self) -> PickGeneration {
        self.generation
    }
}

/// Eight-byte token emitted by the GPU: resident page plus chunk-local row.
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuPickToken {
    page: u32,
    local_row: u32,
}

impl GpuPickToken {
    /// Sentinel used to clear a picking attachment.
    pub const NONE: Self = Self {
        page: u32::MAX,
        local_row: u32::MAX,
    };

    /// Creates the exact two-word value written by a picking attachment.
    #[must_use]
    pub const fn new(resident_page: u32, local_row: u32) -> Self {
        Self {
            page: resident_page,
            local_row,
        }
    }

    /// Resident page encoded by the GPU.
    #[must_use]
    pub const fn page(self) -> Option<ResidentPage> {
        if self.page == u32::MAX {
            None
        } else {
            Some(ResidentPage(self.page))
        }
    }

    /// Chunk-local row encoded by the GPU.
    #[must_use]
    pub const fn local_row(self) -> LocalRow {
        LocalRow::new(self.local_row)
    }
}

/// GPU token paired with the page generation captured at draw submission.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PickReadback {
    token: GpuPickToken,
    generation: PickGeneration,
}

impl PickReadback {
    /// Pairs GPU bytes with the generation retained by the submission.
    #[must_use]
    pub const fn new(token: GpuPickToken, generation: PickGeneration) -> Self {
        Self { token, generation }
    }

    /// Compact bytes returned by the GPU.
    #[must_use]
    pub const fn token(self) -> GpuPickToken {
        self.token
    }

    /// Page generation captured before command submission.
    #[must_use]
    pub const fn generation(self) -> PickGeneration {
        self.generation
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum PageState {
    Vacant,
    Pending(PickPageDescriptor),
    Resident(PickPageDescriptor),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct PageSlot {
    generation: PickGeneration,
    state: PageState,
}

impl Default for PageSlot {
    fn default() -> Self {
        Self {
            generation: PickGeneration(0),
            state: PageState::Vacant,
        }
    }
}

/// Fixed-capacity page table whose memory is proportional to the working set.
#[derive(Debug)]
pub struct PagedPickResolver {
    slots: Box<[PageSlot]>,
    resident: u32,
}

impl PagedPickResolver {
    /// Allocates the complete resolver once; later transitions do not allocate.
    ///
    /// # Errors
    ///
    /// Returns a typed capacity or allocation error.
    pub fn new(capacity: u32) -> Result<Self, PickingError> {
        if capacity == 0 {
            return Err(PickingError::EmptyCapacity);
        }
        if capacity == u32::MAX {
            return Err(PickingError::CapacityTooLarge);
        }
        let capacity = usize::try_from(capacity).map_err(|_| PickingError::CapacityTooLarge)?;
        let mut slots = Vec::new();
        slots
            .try_reserve_exact(capacity)
            .map_err(|_| PickingError::AllocationFailed)?;
        slots.resize(capacity, PageSlot::default());
        Ok(Self {
            slots: slots.into_boxed_slice(),
            resident: 0,
        })
    }

    /// Maximum number of requested and resident pages.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.slots.len()
    }

    /// Number of pages currently ready for picking.
    #[must_use]
    pub const fn resident_len(&self) -> u32 {
        self.resident
    }

    /// Reserves a vacant slot for an asynchronous page upload.
    ///
    /// # Errors
    ///
    /// Rejects duplicate provenance, a full working set or generation overflow.
    pub fn request(
        &mut self,
        descriptor: PickPageDescriptor,
    ) -> Result<PickPageTicket, PickingError> {
        if self.slots.iter().any(|slot| match slot.state {
            PageState::Pending(value) | PageState::Resident(value) => {
                same_namespace(value, descriptor)
            }
            PageState::Vacant => false,
        }) {
            return Err(PickingError::DuplicatePage);
        }
        let Some((index, slot)) = self
            .slots
            .iter_mut()
            .enumerate()
            .find(|(_, slot)| slot.state == PageState::Vacant)
        else {
            return Err(PickingError::WorkingSetFull);
        };
        let Some(generation) = slot.generation.0.checked_add(1) else {
            return Err(PickingError::GenerationExhausted);
        };
        let page = u32::try_from(index).map_err(|_| PickingError::CapacityTooLarge)?;
        slot.generation = PickGeneration(generation);
        slot.state = PageState::Pending(descriptor);
        Ok(PickPageTicket {
            page: ResidentPage(page),
            generation: slot.generation,
        })
    }

    /// Marks one requested page ready after its upload completes.
    ///
    /// # Errors
    ///
    /// Rejects stale completions and pages not awaiting an upload.
    pub fn complete(&mut self, ticket: PickPageTicket) -> Result<(), PickingError> {
        let slot = self.slot_mut(ticket)?;
        let PageState::Pending(descriptor) = slot.state else {
            return Err(PickingError::PageNotPending);
        };
        slot.state = PageState::Resident(descriptor);
        self.resident = self
            .resident
            .checked_add(1)
            .ok_or(PickingError::CapacityTooLarge)?;
        Ok(())
    }

    /// Cancels pending work or evicts a resident page.
    ///
    /// # Errors
    ///
    /// Rejects tickets for another page generation.
    pub fn release(&mut self, ticket: PickPageTicket) -> Result<(), PickingError> {
        let was_resident = {
            let slot = self.slot_mut(ticket)?;
            let was_resident = matches!(slot.state, PageState::Resident(_));
            slot.state = PageState::Vacant;
            was_resident
        };
        if was_resident {
            self.resident = self
                .resident
                .checked_sub(1)
                .ok_or(PickingError::PageNotResident)?;
        }
        Ok(())
    }

    /// Creates GPU bytes for a resident page and validates its local row.
    ///
    /// # Errors
    ///
    /// Rejects stale tickets, non-resident pages and rows outside the chunk.
    pub fn token(
        &self,
        ticket: PickPageTicket,
        local_row: LocalRow,
    ) -> Result<GpuPickToken, PickingError> {
        let descriptor = self.resident_descriptor(ticket.page, ticket.generation)?;
        validate_local(ticket.page, local_row, descriptor.rows())?;
        Ok(GpuPickToken {
            page: ticket.page.get(),
            local_row: local_row.get(),
        })
    }

    /// Resolves a generation-stamped GPU readback to global provenance.
    ///
    /// # Errors
    ///
    /// Rejects empty, stale, non-resident or out-of-span tokens.
    pub fn resolve(&self, readback: PickReadback) -> Result<GlobalPickIdentity, PickingError> {
        let Some(page) = readback.token.page() else {
            return Err(PickingError::EmptyToken);
        };
        let descriptor = self.resident_descriptor(page, readback.generation)?;
        let local = readback.token.local_row();
        validate_local(page, local, descriptor.rows())?;
        let row = descriptor
            .rows()
            .logical_row(descriptor.chunk(), local)
            .map_err(|_| local_error(page, local, descriptor.rows()))?;
        Ok(GlobalPickIdentity {
            dataset: descriptor.dataset(),
            chunk: descriptor.chunk(),
            row,
            kind: descriptor.kind(),
        })
    }

    fn resident_descriptor(
        &self,
        page: ResidentPage,
        generation: PickGeneration,
    ) -> Result<PickPageDescriptor, PickingError> {
        let slot = self.slot(page)?;
        if slot.generation != generation {
            return Err(PickingError::StaleGeneration);
        }
        match slot.state {
            PageState::Resident(descriptor) => Ok(descriptor),
            PageState::Vacant | PageState::Pending(_) => Err(PickingError::PageNotResident),
        }
    }

    fn slot(&self, page: ResidentPage) -> Result<&PageSlot, PickingError> {
        let index = usize::try_from(page.get()).map_err(|_| page_error(page))?;
        self.slots.get(index).ok_or_else(|| page_error(page))
    }

    fn slot_mut(&mut self, ticket: PickPageTicket) -> Result<&mut PageSlot, PickingError> {
        let index = usize::try_from(ticket.page.get()).map_err(|_| page_error(ticket.page))?;
        let slot = self
            .slots
            .get_mut(index)
            .ok_or_else(|| page_error(ticket.page))?;
        if slot.generation != ticket.generation {
            return Err(PickingError::StaleGeneration);
        }
        Ok(slot)
    }
}

fn validate_local(page: ResidentPage, row: LocalRow, span: ChunkSpan) -> Result<(), PickingError> {
    if row.get() >= span.row_count() {
        return Err(local_error(page, row, span));
    }
    Ok(())
}

fn local_error(page: ResidentPage, row: LocalRow, span: ChunkSpan) -> PickingError {
    PickingError::LocalRowOutsidePage {
        page: page.get(),
        row: row.get(),
        row_count: span.row_count(),
    }
}

fn page_error(page: ResidentPage) -> PickingError {
    PickingError::PageOutOfRange { page: page.get() }
}

fn same_namespace(left: PickPageDescriptor, right: PickPageDescriptor) -> bool {
    left.dataset() == right.dataset()
        && left.chunk() == right.chunk()
        && left.kind() == right.kind()
}

#[cfg(test)]
#[path = "picking_tests.rs"]
mod tests;
