//! Sparse brick identities and immutable logical metadata.

#[path = "brick_index.rs"]
mod brick_index;

use self::brick_index::BrickSpatialIndex;
use crate::{ChunkId, DatasetError, DatasetId};
use core::fmt;
use core::ops::ControlFlow;

/// Globally stable identity of one sparse brick.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct BrickId(u64);

impl BrickId {
    /// Creates an identity from a provider-owned stable value.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the full 64-bit identity.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for BrickId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// Logical grid address of an interior brick at one mip level.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct BrickAddress {
    /// Interior origin in mip-local voxels.
    pub origin: [u64; 3],
    /// Zero is full resolution; larger values are progressively coarser.
    pub mip: u16,
}

/// Monotonic provider generation used for dirty tracking.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub struct DirtyGeneration(u64);

impl DirtyGeneration {
    /// Creates a generation from provider state.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the generation value.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    /// Produces the next dirty generation.
    ///
    /// # Errors
    ///
    /// Returns [`DatasetError::GenerationExhausted`] at `u64::MAX`.
    pub fn next(self) -> Result<Self, DatasetError> {
        self.0
            .checked_add(1)
            .map(Self)
            .ok_or(DatasetError::GenerationExhausted)
    }
}

/// Scientific meaning and conservative value range of a brick.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum BrickValueRange {
    /// Continuous scalar field.
    Scalar {
        /// Smallest scalar value in the brick.
        min: f32,
        /// Largest scalar value in the brick.
        max: f32,
    },
    /// Categorical segmentation labels.
    Segmentation {
        /// Smallest categorical label in the brick.
        min: u32,
        /// Largest categorical label in the brick.
        max: u32,
    },
    /// Bit-packed occupancy values.
    Occupancy {
        /// At least one voxel is empty.
        has_empty: bool,
        /// At least one voxel is occupied.
        has_occupied: bool,
    },
}

/// Validated shape of stored voxels, including a symmetric halo.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct BrickShape {
    stored: [u16; 3],
    interior: [u16; 3],
    halo: u16,
    voxel_count: u32,
}

impl BrickShape {
    /// Validates non-empty interior dimensions and chunk-local voxel count.
    ///
    /// # Errors
    ///
    /// Returns a typed payload error for invalid dimensions or overflow.
    pub fn new(stored: [u16; 3], halo: u16) -> Result<Self, DatasetError> {
        let doubled = halo.checked_mul(2).ok_or(DatasetError::InvalidBrickShape)?;
        let mut interior = [0; 3];
        for (target, dimension) in interior.iter_mut().zip(stored) {
            *target = dimension
                .checked_sub(doubled)
                .filter(|value| *value > 0)
                .ok_or(DatasetError::InvalidBrickShape)?;
        }
        let voxel_count = stored.into_iter().try_fold(1u32, |total, dimension| {
            total.checked_mul(u32::from(dimension))
        });
        Ok(Self {
            stored,
            interior,
            halo,
            voxel_count: voxel_count.ok_or(DatasetError::InvalidBrickShape)?,
        })
    }

    /// Stored dimensions including halo voxels.
    #[must_use]
    pub const fn stored(self) -> [u16; 3] {
        self.stored
    }

    /// Interior dimensions represented in the logical volume.
    #[must_use]
    pub const fn interior(self) -> [u16; 3] {
        self.interior
    }

    /// Symmetric halo width on every face.
    #[must_use]
    pub const fn halo(self) -> u16 {
        self.halo
    }

    /// Number of stored voxels, always addressable by `u32`.
    #[must_use]
    pub const fn voxel_count(self) -> u32 {
        self.voxel_count
    }
}

/// Immutable metadata sufficient to select and validate a brick without data.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct BrickMetadata {
    /// Global brick identity.
    pub id: BrickId,
    /// Logical address and mip.
    pub address: BrickAddress,
    /// Stored and interior dimensions.
    pub shape: BrickShape,
    /// Conservative value range.
    pub range: BrickValueRange,
    /// Dirty generation supplied by the producer.
    pub generation: DirtyGeneration,
}

impl BrickMetadata {
    /// Validates range semantics and finite scalar extrema.
    ///
    /// # Errors
    ///
    /// Returns a typed error for inverted or non-finite ranges.
    pub fn new(
        id: BrickId,
        address: BrickAddress,
        shape: BrickShape,
        range: BrickValueRange,
        generation: DirtyGeneration,
    ) -> Result<Self, DatasetError> {
        let valid = match range {
            BrickValueRange::Scalar { min, max } => {
                min.is_finite() && max.is_finite() && min <= max
            }
            BrickValueRange::Segmentation { min, max } => min <= max,
            BrickValueRange::Occupancy {
                has_empty,
                has_occupied,
            } => has_empty || has_occupied,
        };
        if !valid {
            return Err(DatasetError::InvalidBrickRange);
        }
        Ok(Self {
            id,
            address,
            shape,
            range,
            generation,
        })
    }
}

/// Catalog association for one sparse brick; contains no voxel payload.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct BrickDescriptor {
    /// Independently loadable dataset chunk.
    pub chunk: ChunkId,
    /// Sparse brick metadata.
    pub metadata: BrickMetadata,
}

/// Sparse logical-volume catalog independent of payload residency.
#[derive(Clone, Debug)]
pub struct BrickCatalog {
    dataset: DatasetId,
    logical_extent: [u64; 3],
    voxel_bytes: u16,
    descriptors: Vec<BrickDescriptor>,
    spatial_index: BrickSpatialIndex,
}

impl BrickCatalog {
    /// Builds a sorted sparse catalog without allocating logical voxel storage.
    ///
    /// # Errors
    ///
    /// Rejects duplicate identities, invalid extents and out-of-bounds bricks.
    pub fn new(
        dataset: DatasetId,
        logical_extent: [u64; 3],
        voxel_bytes: u16,
        mut descriptors: Vec<BrickDescriptor>,
    ) -> Result<Self, DatasetError> {
        if logical_extent.contains(&0) || voxel_bytes == 0 {
            return Err(DatasetError::InvalidLogicalVolume);
        }
        descriptors.sort_unstable_by_key(|value| value.metadata.id);
        if descriptors
            .windows(2)
            .any(|pair| pair[0].metadata.id == pair[1].metadata.id)
        {
            return Err(DatasetError::DuplicateBrick);
        }
        for descriptor in &descriptors {
            validate_bounds(logical_extent, descriptor.metadata)?;
        }
        let spatial_index = BrickSpatialIndex::build(&mut descriptors)?;
        Ok(Self {
            dataset,
            logical_extent,
            voxel_bytes,
            descriptors,
            spatial_index,
        })
    }

    /// Owning dataset.
    #[must_use]
    pub const fn dataset_id(&self) -> DatasetId {
        self.dataset
    }

    /// Full-resolution logical dimensions.
    #[must_use]
    pub const fn logical_extent(&self) -> [u64; 3] {
        self.logical_extent
    }

    /// Sparse metadata rows in compact spatial order, grouped by mip.
    #[must_use]
    pub fn descriptors(&self) -> &[BrickDescriptor] {
        &self.descriptors
    }

    /// Visits candidate descriptors at `mip` from intersecting spatial leaves.
    ///
    /// The visitor receives the exact distance for matches and `None` for leaf
    /// false positives. Its call count is therefore `visited_descriptors`.
    /// Traversal is stackless and performs no allocation. The return value is
    /// the number of visited hierarchy nodes. Returning [`ControlFlow::Break`]
    /// from `visitor` stops the query early.
    #[must_use]
    pub fn query_mip<F>(&self, mip: u16, focus: [u64; 3], radius: u64, visitor: F) -> u32
    where
        F: FnMut(BrickDescriptor, Option<u64>) -> ControlFlow<()>,
    {
        let mip_focus = focus.map(|value| value >> mip);
        self.spatial_index
            .query(&self.descriptors, mip, mip_focus, radius, visitor)
    }

    /// Logical dense size used for accounting only; no payload is materialized.
    ///
    /// # Errors
    ///
    /// Returns [`DatasetError::LogicalVolumeOverflow`] if bytes exceed `u64`.
    pub fn logical_bytes(&self) -> Result<u64, DatasetError> {
        self.logical_extent
            .into_iter()
            .try_fold(1u64, u64::checked_mul)
            .and_then(|voxels| voxels.checked_mul(u64::from(self.voxel_bytes)))
            .ok_or(DatasetError::LogicalVolumeOverflow)
    }
}

fn validate_bounds(extent: [u64; 3], metadata: BrickMetadata) -> Result<(), DatasetError> {
    for ((origin, interior), full) in metadata
        .address
        .origin
        .into_iter()
        .zip(metadata.shape.interior())
        .zip(extent)
    {
        let scale = 1u64
            .checked_shl(u32::from(metadata.address.mip))
            .ok_or(DatasetError::InvalidLogicalVolume)?;
        let end = origin
            .checked_add(u64::from(interior))
            .and_then(|value| value.checked_mul(scale))
            .ok_or(DatasetError::InvalidLogicalVolume)?;
        if end > full {
            return Err(DatasetError::BrickOutsideLogicalVolume);
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "brick_tests.rs"]
mod tests;
