//! Immutable native-width storage admitted by the generic residency path.

use super::PagedRelation;
use crate::{AttributeKind, AttributeValues, ChunkDomainRef, DatasetError, RigidInstance};
use std::sync::Arc;

/// Tightly packed 12-byte point positions.
#[derive(Clone, PartialEq, Debug)]
pub struct PointChunkPayload {
    positions: Arc<[[f32; 3]]>,
}

impl PointChunkPayload {
    /// Validates finite, non-empty positions without copying them.
    ///
    /// # Errors
    ///
    /// Returns a typed error for empty, non-finite or over-large input.
    pub fn new(positions: Arc<[[f32; 3]]>) -> Result<Self, DatasetError> {
        checked_rows(positions.len())?;
        if positions.is_empty() || positions.iter().flatten().any(|value| !value.is_finite()) {
            return Err(invalid(
                "point chunk positions must be non-empty and finite",
            ));
        }
        Ok(Self { positions })
    }

    /// Shared positions suitable for direct upload.
    #[must_use]
    pub const fn positions(&self) -> &Arc<[[f32; 3]]> {
        &self.positions
    }

    pub(in crate::dataset) fn row_count(&self) -> Result<u32, DatasetError> {
        checked_rows(self.positions.len())
    }

    pub(in crate::dataset) fn minimum_host_bytes(&self) -> Result<u64, DatasetError> {
        bytes(self.positions.len(), 12)
    }
}

/// Tightly packed 32-byte rigid transforms sharing a placement template.
#[derive(Clone, PartialEq, Debug)]
pub struct InstanceChunkPayload {
    transforms: Arc<[RigidInstance]>,
}

impl InstanceChunkPayload {
    /// Retains a non-empty validated transform stream without copying it.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the stream exceeds chunk-local addressing.
    pub fn new(transforms: Arc<[RigidInstance]>) -> Result<Self, DatasetError> {
        checked_rows(transforms.len())?;
        if transforms.is_empty() {
            return Err(invalid("instance chunk transforms must be non-empty"));
        }
        Ok(Self { transforms })
    }

    /// Shared transforms suitable for direct upload.
    #[must_use]
    pub const fn transforms(&self) -> &Arc<[RigidInstance]> {
        &self.transforms
    }

    pub(in crate::dataset) fn row_count(&self) -> Result<u32, DatasetError> {
        checked_rows(self.transforms.len())
    }

    pub(in crate::dataset) fn minimum_host_bytes(&self) -> Result<u64, DatasetError> {
        bytes(self.transforms.len(), 32)
    }
}

/// Globally anchored relation rows lowered only after dependency residency.
#[derive(Clone, PartialEq, Debug)]
pub struct RelationChunkPayload {
    relations: Arc<[PagedRelation]>,
}

impl RelationChunkPayload {
    /// Validates non-empty anchors without resolving their external sources.
    ///
    /// # Errors
    ///
    /// Returns a typed error for invalid world positions, template references
    /// or row counts beyond chunk-local addressing.
    pub fn new(relations: Arc<[PagedRelation]>) -> Result<Self, DatasetError> {
        checked_rows(relations.len())?;
        if relations.is_empty()
            || relations
                .iter()
                .flat_map(|relation| [relation.start, relation.end])
                .any(|anchor| !anchor.valid())
        {
            return Err(invalid("relation chunk anchors must be spatial and valid"));
        }
        Ok(Self { relations })
    }

    /// Shared logical relation rows.
    #[must_use]
    pub const fn relations(&self) -> &Arc<[PagedRelation]> {
        &self.relations
    }

    pub(in crate::dataset) fn row_count(&self) -> Result<u32, DatasetError> {
        checked_rows(self.relations.len())
    }

    pub(in crate::dataset) fn minimum_host_bytes(&self) -> Result<u64, DatasetError> {
        bytes(
            self.relations.len(),
            std::mem::size_of::<PagedRelation>() as u64,
        )
    }
}

/// One native-width typed column aligned to another chunk's logical rows.
#[derive(Clone, PartialEq, Debug)]
pub struct AttributeChunkPayload {
    target: ChunkDomainRef,
    values: AttributeValues,
}

impl AttributeChunkPayload {
    /// Validates one non-empty scalar/category/vector/color column.
    ///
    /// # Errors
    ///
    /// Returns a typed error for malformed values or chunk-local overflow.
    pub fn new(target: ChunkDomainRef, values: AttributeValues) -> Result<Self, DatasetError> {
        checked_rows(values.len())?;
        if values.is_empty() || values.validate().is_err() {
            return Err(invalid(
                "attribute chunk values must be non-empty and valid",
            ));
        }
        Ok(Self { target, values })
    }

    /// Exact scene-independent table receiving this column.
    #[must_use]
    pub const fn target(&self) -> ChunkDomainRef {
        self.target
    }

    /// Native physical layout.
    #[must_use]
    pub const fn kind(&self) -> AttributeKind {
        self.values.kind()
    }

    /// Shared values suitable for a single direct arena upload.
    #[must_use]
    pub const fn values(&self) -> &AttributeValues {
        &self.values
    }

    pub(in crate::dataset) fn row_count(&self) -> Result<u32, DatasetError> {
        checked_rows(self.values.len())
    }

    pub(in crate::dataset) fn minimum_host_bytes(&self) -> Result<u64, DatasetError> {
        bytes(self.values.len(), u64::from(self.values.kind().stride()))
    }
}

fn checked_rows(rows: usize) -> Result<u32, DatasetError> {
    u32::try_from(rows).map_err(|_| invalid("generic chunk exceeds local u32 addressing"))
}

fn bytes(rows: usize, stride: u64) -> Result<u64, DatasetError> {
    u64::try_from(rows)
        .ok()
        .and_then(|rows| rows.checked_mul(stride))
        .ok_or(DatasetError::PayloadByteSizeOverflow)
}

const fn invalid(reason: &'static str) -> DatasetError {
    DatasetError::InvalidPayload { reason }
}
