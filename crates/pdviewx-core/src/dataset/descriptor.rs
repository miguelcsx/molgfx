//! Compact metadata needed to choose chunks before loading their payloads.

use crate::{ChunkFootprint, ChunkId, DatasetError, LocalRow, LogicalRow};

/// Host-neutral payload category declared by a chunk descriptor.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum PayloadKind {
    /// Molecular coordinates and immutable atom columns.
    Structure,
    /// Chemical bond rows whose atom endpoints remain globally addressable.
    BondTopology,
    /// Scalar values aligned to logical rows.
    ScalarProperty,
    /// One decoded trajectory frame segment.
    Trajectory,
    /// One scalar volume brick.
    VolumeBrick,
    /// One categorical label brick.
    LabelBrick,
    /// Indexed triangle geometry.
    Mesh,
    /// Coarse geometry retained while detail is absent.
    Proxy,
    /// Generic tightly packed point positions.
    PointBatch,
    /// Generic 32-byte rigid transforms sharing an external template.
    InstanceBatch,
    /// Generic spatial relations with globally identified anchors.
    RelationBatch,
    /// One native-width typed visual attribute column.
    Attribute,
}

/// Half-open logical-row interval represented by one chunk.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ChunkSpan {
    first: LogicalRow,
    row_count: u32,
}

impl ChunkSpan {
    /// Creates a non-empty span whose exclusive end fits in `u64`.
    ///
    /// # Errors
    ///
    /// Returns a typed error for an empty or overflowing interval.
    pub fn new(first: LogicalRow, row_count: u32) -> Result<Self, DatasetError> {
        if row_count == 0 {
            return Err(DatasetError::EmptyRowSpan);
        }
        first
            .get()
            .checked_add(u64::from(row_count))
            .ok_or(DatasetError::RowSpanOverflow)?;
        Ok(Self { first, row_count })
    }

    /// First logical row in the interval.
    #[must_use]
    pub const fn first(self) -> LogicalRow {
        self.first
    }

    /// Number of rows addressable by `LocalRow`.
    #[must_use]
    pub const fn row_count(self) -> u32 {
        self.row_count
    }

    /// Exclusive logical end. Construction guarantees this cannot overflow.
    #[must_use]
    pub fn end(self) -> LogicalRow {
        LogicalRow::new(self.first.get() + u64::from(self.row_count))
    }

    /// Converts a global row to its compact chunk-local index.
    ///
    /// # Errors
    ///
    /// Returns [`DatasetError::LogicalRowOutsideChunk`] for another chunk's row.
    pub fn local_row(self, chunk: ChunkId, row: LogicalRow) -> Result<LocalRow, DatasetError> {
        let Some(offset) = row.get().checked_sub(self.first.get()) else {
            return Err(outside(chunk, row));
        };
        if offset >= u64::from(self.row_count) {
            return Err(outside(chunk, row));
        }
        let Ok(offset) = u32::try_from(offset) else {
            return Err(outside(chunk, row));
        };
        Ok(LocalRow::new(offset))
    }

    /// Converts a local row back to its global identity.
    ///
    /// # Errors
    ///
    /// Returns [`DatasetError::LogicalRowOutsideChunk`] when `row` exceeds
    /// this chunk's local range.
    pub fn logical_row(self, chunk: ChunkId, row: LocalRow) -> Result<LogicalRow, DatasetError> {
        if row.get() >= self.row_count {
            return Err(outside(chunk, self.end()));
        }
        Ok(LogicalRow::new(self.first.get() + u64::from(row.get())))
    }
}

fn outside(chunk: ChunkId, row: LogicalRow) -> DatasetError {
    DatasetError::LogicalRowOutsideChunk { chunk, row }
}

/// Finite axis-aligned bounds represented without backend-specific math types.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ChunkBounds {
    /// Lower corner in dataset coordinates.
    pub min: [f32; 3],
    /// Upper corner in dataset coordinates.
    pub max: [f32; 3],
}

impl ChunkBounds {
    /// Validates finite, ordered bounds.
    ///
    /// # Errors
    ///
    /// Returns [`DatasetError::InvalidBounds`] for non-finite or inverted
    /// bounds.
    pub fn new(min: [f32; 3], max: [f32; 3]) -> Result<Self, DatasetError> {
        if min
            .iter()
            .zip(max)
            .any(|(lower, upper)| !lower.is_finite() || !upper.is_finite() || *lower > upper)
        {
            return Err(DatasetError::InvalidBounds);
        }
        Ok(Self { min, max })
    }
}

/// Immutable metadata for one independently loadable chunk.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ChunkDescriptor {
    /// Stable identity within the dataset.
    pub id: ChunkId,
    /// Coarser parent, absent for a root chunk.
    pub parent: Option<ChunkId>,
    /// Root is zero; every child is exactly one level deeper.
    pub level: u16,
    /// Logical rows represented by this payload.
    pub rows: ChunkSpan,
    /// Conservative dataset-space bounds.
    pub bounds: ChunkBounds,
    /// Payload category expected from the caller.
    pub payload_kind: PayloadKind,
    /// Memory estimates used before payload arrival.
    pub footprint: ChunkFootprint,
}

#[cfg(test)]
#[path = "descriptor_tests.rs"]
mod tests;
