//! Shared sparse volume, segmentation and occupancy payloads.

use super::payload::invalid;
use crate::{BrickMetadata, BrickValueRange, DatasetError};
use std::sync::Arc;

/// One continuous scalar brick with caller-owned shared storage.
#[derive(Clone, Debug)]
pub struct VolumeBrickPayload {
    metadata: BrickMetadata,
    values: Arc<[f32]>,
}

impl VolumeBrickPayload {
    /// Validates semantic type, exact storage length and finite values.
    ///
    /// # Errors
    ///
    /// Returns a typed payload error if metadata and storage disagree.
    pub fn new(metadata: BrickMetadata, values: Arc<[f32]>) -> Result<Self, DatasetError> {
        let BrickValueRange::Scalar { min, max } = metadata.range else {
            return Err(invalid("volume brick requires a scalar range"));
        };
        validate_len(metadata, values.len())?;
        if values
            .iter()
            .any(|value| !value.is_finite() || *value < min || *value > max)
        {
            return Err(invalid(
                "volume brick values must be finite and inside the declared range",
            ));
        }
        Ok(Self { metadata, values })
    }

    /// Selection and dirty-tracking metadata.
    #[must_use]
    pub const fn metadata(&self) -> BrickMetadata {
        self.metadata
    }

    /// Shared x-fastest scalar storage, including halo voxels.
    #[must_use]
    pub fn values(&self) -> &Arc<[f32]> {
        &self.values
    }
}

/// One categorical segmentation brick with shared labels.
#[derive(Clone, Debug)]
pub struct LabelBrickPayload {
    metadata: BrickMetadata,
    labels: Arc<[u32]>,
}

impl LabelBrickPayload {
    /// Validates segmentation semantics and exact storage length.
    ///
    /// # Errors
    ///
    /// Returns a typed payload error if metadata and storage disagree.
    pub fn new(metadata: BrickMetadata, labels: Arc<[u32]>) -> Result<Self, DatasetError> {
        let BrickValueRange::Segmentation { min, max } = metadata.range else {
            return Err(invalid("label brick requires a segmentation range"));
        };
        validate_len(metadata, labels.len())?;
        if labels.iter().any(|value| *value < min || *value > max) {
            return Err(invalid("labels must be inside the declared range"));
        }
        Ok(Self { metadata, labels })
    }

    /// Selection and dirty-tracking metadata.
    #[must_use]
    pub const fn metadata(&self) -> BrickMetadata {
        self.metadata
    }

    /// Shared x-fastest categorical storage, including halo voxels.
    #[must_use]
    pub fn labels(&self) -> &Arc<[u32]> {
        &self.labels
    }
}

/// One bit-packed occupancy brick; one bit addresses one stored voxel.
#[derive(Clone, Debug)]
pub struct OccupancyBrickPayload {
    metadata: BrickMetadata,
    words: Arc<[u64]>,
}

impl OccupancyBrickPayload {
    /// Validates occupancy semantics and exact bit capacity.
    ///
    /// # Errors
    ///
    /// Returns a typed payload error if metadata and storage disagree.
    pub fn new(metadata: BrickMetadata, words: Arc<[u64]>) -> Result<Self, DatasetError> {
        let BrickValueRange::Occupancy {
            has_empty,
            has_occupied,
        } = metadata.range
        else {
            return Err(invalid("occupancy brick requires an occupancy range"));
        };
        let voxels = u64::from(metadata.shape.voxel_count());
        let expected = voxels
            .checked_add(63)
            .map(|value| value / 64)
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| invalid("occupancy word count exceeds host addressing"))?;
        if words.len() != expected {
            return Err(invalid("occupancy word count does not match brick shape"));
        }
        validate_occupancy(words.as_ref(), metadata.shape.voxel_count())?;
        let occupied = words.iter().any(|word| *word != 0);
        let empty = occupied_count(words.as_ref()) < voxels;
        if occupied != has_occupied || empty != has_empty {
            return Err(invalid(
                "occupancy extrema do not match the packed voxel values",
            ));
        }
        Ok(Self { metadata, words })
    }

    /// Selection and dirty-tracking metadata.
    #[must_use]
    pub const fn metadata(&self) -> BrickMetadata {
        self.metadata
    }

    /// Shared bit-packed storage, including halo voxels.
    #[must_use]
    pub fn words(&self) -> &Arc<[u64]> {
        &self.words
    }

    /// Number of stored voxels represented by the packed words.
    #[must_use]
    pub const fn voxel_count(&self) -> u32 {
        self.metadata.shape.voxel_count()
    }
}

fn validate_occupancy(words: &[u64], voxels: u32) -> Result<(), DatasetError> {
    let used = voxels % 64;
    let Some(last) = words.last().copied() else {
        return Err(invalid("occupancy storage must be non-empty"));
    };
    if used != 0 && last >> used != 0 {
        return Err(invalid("unused occupancy bits must be zero"));
    }
    Ok(())
}

fn occupied_count(words: &[u64]) -> u64 {
    words.iter().map(|word| u64::from(word.count_ones())).sum()
}

fn validate_len(metadata: BrickMetadata, actual: usize) -> Result<(), DatasetError> {
    let expected = usize::try_from(metadata.shape.voxel_count())
        .map_err(|_| invalid("brick voxel count exceeds host addressing"))?;
    if expected != actual {
        return Err(invalid("brick value count does not match shape"));
    }
    Ok(())
}
