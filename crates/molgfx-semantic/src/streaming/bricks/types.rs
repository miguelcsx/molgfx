//! Public sparse-brick residency and selection records.

use molgfx_core::{BrickAddress, BrickDescriptor, BrickId, BrickMetadata, ChunkId};

/// Compact atlas index valid only inside one working set.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct AtlasSlot(u32);

impl AtlasSlot {
    pub(super) const fn new(value: u32) -> Self {
        Self(value)
    }

    /// Returns the page-table-compatible local index.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// One logical-to-atlas mapping retained in the working set.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct BrickPage {
    /// Global brick identity and immutable metadata.
    pub metadata: BrickMetadata,
    /// Provider chunk used to refresh this page.
    pub chunk: ChunkId,
    /// Compact atlas allocation.
    pub slot: AtlasSlot,
}

impl BrickPage {
    /// Logical coordinate used by a sparse page table.
    #[must_use]
    pub const fn address(self) -> BrickAddress {
        self.metadata.address
    }
}

/// One clipmap candidate, ordered independently of payload storage.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct BrickSelection {
    /// Catalog descriptor to request or retain.
    pub descriptor: BrickDescriptor,
    /// Existing atlas slot, absent when the provider must supply the brick.
    pub resident_slot: Option<AtlasSlot>,
    /// Integer Chebyshev distance in mip-local voxels.
    pub distance: u64,
}

/// Work counters from one bounded clipmap metadata query.
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct ClipmapQueryStats {
    /// Hierarchy nodes whose bounds were tested.
    pub visited_nodes: u32,
    /// Descriptors tested in intersecting leaves.
    pub visited_descriptors: u32,
    /// Descriptors written to selection scratch.
    pub matched_descriptors: u32,
}

/// Caller-owned fixed-capacity output reused across clipmap traversals.
#[derive(Debug)]
pub struct BrickSelectionScratch {
    entries: Vec<BrickSelection>,
    limit: usize,
}

impl BrickSelectionScratch {
    /// Allocates selection storage once. Traversal never grows beyond `limit`.
    #[must_use]
    pub fn with_capacity(limit: usize) -> Self {
        Self {
            entries: Vec::with_capacity(limit),
            limit,
        }
    }

    /// Current ordered selections.
    #[must_use]
    pub fn entries(&self) -> &[BrickSelection] {
        &self.entries
    }

    pub(super) fn clear(&mut self) {
        self.entries.clear();
    }

    pub(super) fn push(&mut self, value: BrickSelection) -> Result<(), BrickWorkingSetError> {
        if self.entries.len() == self.limit {
            return Err(BrickWorkingSetError::CapacityExceeded {
                resource: "clipmap selection scratch",
                capacity: self.limit,
            });
        }
        self.entries.push(value);
        Ok(())
    }

    pub(super) fn sort(&mut self) {
        self.entries.sort_unstable_by_key(|entry| {
            (
                entry.descriptor.metadata.address.mip,
                entry.distance,
                entry.descriptor.metadata.id,
            )
        });
    }
}

/// One clipmap ring expressed in mip-local voxels.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ClipmapLevel {
    /// Catalog mip selected by this ring.
    pub mip: u16,
    /// Chebyshev radius around the focus point.
    pub radius: u64,
}

/// Typed sparse-working-set failures; no hidden capacity growth occurs.
#[derive(Clone, Copy, PartialEq, Eq, Debug, thiserror::Error)]
pub enum BrickWorkingSetError {
    /// A fixed-capacity working buffer cannot admit another item.
    #[error("{resource} capacity {capacity} is exhausted")]
    CapacityExceeded {
        /// Fixed resource that applied backpressure.
        resource: &'static str,
        /// Configured item capacity.
        capacity: usize,
    },
    /// A completion predates the currently mapped dirty generation.
    #[error("brick {brick} generation {received} is stale; current is {current}")]
    StaleGeneration {
        /// Global brick identity.
        brick: BrickId,
        /// Current mapped generation.
        current: u64,
        /// Rejected generation.
        received: u64,
    },
    /// Equal generations carried conflicting immutable metadata.
    #[error("brick {brick} reused a generation with conflicting metadata")]
    GenerationConflict {
        /// Global brick identity.
        brick: BrickId,
    },
    /// Another global identity already owns the logical address.
    #[error("logical brick address is already mapped by {brick}")]
    AddressOccupied {
        /// Existing global brick identity.
        brick: BrickId,
    },
    /// The requested identity is not resident.
    #[error("brick {brick} is not resident")]
    NotResident {
        /// Missing global brick identity.
        brick: BrickId,
    },
    /// Clipmap mip levels are duplicated or radii are not monotonic.
    #[error("clipmap levels must have unique mips and nondecreasing radii")]
    InvalidClipmap,
}
