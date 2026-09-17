//! Fixed-capacity logical page table with deterministic slot reuse.

use super::{AtlasSlot, BrickPage, BrickWorkingSetError};
use molgfx_core::{BrickAddress, BrickDescriptor, BrickId};

/// Sparse atlas/page-table state bounded by the configured resident capacity.
#[derive(Debug)]
pub struct BrickWorkingSet {
    pages: Vec<BrickPage>,
    free_slots: Vec<AtlasSlot>,
    capacity: usize,
}

impl BrickWorkingSet {
    /// Allocates all page-table and free-list storage up front.
    ///
    /// # Errors
    ///
    /// Rejects capacities that cannot be addressed by an atlas-local `u32`.
    pub fn new(capacity: usize) -> Result<Self, BrickWorkingSetError> {
        let Ok(local_capacity) = u32::try_from(capacity) else {
            return Err(BrickWorkingSetError::CapacityExceeded {
                resource: "u32 atlas addressing",
                capacity,
            });
        };
        let mut free_slots = Vec::with_capacity(capacity);
        for slot in (0..local_capacity).rev() {
            free_slots.push(AtlasSlot::new(slot));
        }
        Ok(Self {
            pages: Vec::with_capacity(capacity),
            free_slots,
            capacity,
        })
    }

    /// Maximum number of resident bricks.
    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    /// Current logical page-table entries, sorted by global identity.
    #[must_use]
    pub fn pages(&self) -> &[BrickPage] {
        &self.pages
    }

    /// Finds a resident page by global identity in logarithmic time.
    #[must_use]
    pub fn get(&self, id: BrickId) -> Option<&BrickPage> {
        self.pages
            .binary_search_by_key(&id, |page| page.metadata.id)
            .ok()
            .and_then(|index| self.pages.get(index))
    }

    /// Resolves a logical coordinate/mip to a resident atlas page.
    #[must_use]
    pub fn resolve(&self, address: BrickAddress) -> Option<&BrickPage> {
        self.pages
            .iter()
            .find(|page| page.metadata.address == address)
    }

    /// Commits a validated provider generation without growing capacity.
    ///
    /// Equal identical generations are idempotent. Newer generations retain
    /// their atlas slot. Older completions and logical collisions are rejected.
    ///
    /// # Errors
    ///
    /// Returns typed stale, conflict, address or capacity errors.
    pub fn commit(
        &mut self,
        descriptor: BrickDescriptor,
    ) -> Result<AtlasSlot, BrickWorkingSetError> {
        let id = descriptor.metadata.id;
        if let Ok(index) = self
            .pages
            .binary_search_by_key(&id, |page| page.metadata.id)
        {
            return self.refresh(index, descriptor);
        }
        if let Some(page) = self.resolve(descriptor.metadata.address) {
            return Err(BrickWorkingSetError::AddressOccupied {
                brick: page.metadata.id,
            });
        }
        let Some(slot) = self.free_slots.pop() else {
            return Err(BrickWorkingSetError::CapacityExceeded {
                resource: "brick atlas",
                capacity: self.capacity,
            });
        };
        let page = BrickPage {
            metadata: descriptor.metadata,
            chunk: descriptor.chunk,
            slot,
        };
        let index = match self
            .pages
            .binary_search_by_key(&id, |value| value.metadata.id)
        {
            Ok(index) | Err(index) => index,
        };
        self.pages.insert(index, page);
        Ok(slot)
    }

    /// Evicts one page and makes its slot available for deterministic reuse.
    ///
    /// # Errors
    ///
    /// Returns [`BrickWorkingSetError::NotResident`] for an absent identity.
    pub fn evict(&mut self, id: BrickId) -> Result<BrickPage, BrickWorkingSetError> {
        let index = self
            .pages
            .binary_search_by_key(&id, |page| page.metadata.id)
            .map_err(|_| BrickWorkingSetError::NotResident { brick: id })?;
        let page = self.pages.remove(index);
        self.free_slots.push(page.slot);
        Ok(page)
    }

    fn refresh(
        &mut self,
        index: usize,
        descriptor: BrickDescriptor,
    ) -> Result<AtlasSlot, BrickWorkingSetError> {
        let Some(current) = self.pages.get(index).copied() else {
            return Err(BrickWorkingSetError::NotResident {
                brick: descriptor.metadata.id,
            });
        };
        let received = descriptor.metadata.generation.get();
        let active = current.metadata.generation.get();
        if received < active {
            return Err(BrickWorkingSetError::StaleGeneration {
                brick: descriptor.metadata.id,
                current: active,
                received,
            });
        }
        if received == active {
            if current.metadata != descriptor.metadata || current.chunk != descriptor.chunk {
                return Err(BrickWorkingSetError::GenerationConflict {
                    brick: descriptor.metadata.id,
                });
            }
            return Ok(current.slot);
        }
        if let Some(page) = self.resolve(descriptor.metadata.address)
            && page.metadata.id != descriptor.metadata.id
        {
            return Err(BrickWorkingSetError::AddressOccupied {
                brick: page.metadata.id,
            });
        }
        self.pages[index] = BrickPage {
            metadata: descriptor.metadata,
            chunk: descriptor.chunk,
            slot: current.slot,
        };
        Ok(current.slot)
    }
}
