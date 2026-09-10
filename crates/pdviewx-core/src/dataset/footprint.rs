//! Byte budgets carried by chunk metadata without allocating payloads.

use crate::DatasetError;

/// Estimated bytes consumed by one chunk at each residency boundary.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct ChunkFootprint {
    /// Bytes in caller-owned source storage.
    pub source_bytes: u64,
    /// Decoded bytes retained in host memory.
    pub host_bytes: u64,
    /// Temporary bytes needed while uploading.
    pub staging_bytes: u64,
    /// Bytes retained by the GPU-resident representation.
    pub gpu_bytes: u64,
}

impl ChunkFootprint {
    /// Constructs a footprint without allocating any storage.
    #[must_use]
    pub const fn new(
        source_bytes: u64,
        host_bytes: u64,
        staging_bytes: u64,
        gpu_bytes: u64,
    ) -> Self {
        Self {
            source_bytes,
            host_bytes,
            staging_bytes,
            gpu_bytes,
        }
    }

    /// Sums every residency class with checked arithmetic.
    ///
    /// # Errors
    ///
    /// Returns [`DatasetError::FootprintOverflow`] when the total exceeds
    /// `u64::MAX`.
    pub fn total_bytes(self) -> Result<u64, DatasetError> {
        self.source_bytes
            .checked_add(self.host_bytes)
            .and_then(|value| value.checked_add(self.staging_bytes))
            .and_then(|value| value.checked_add(self.gpu_bytes))
            .ok_or(DatasetError::FootprintOverflow)
    }

    pub(crate) fn checked_add(self, other: Self) -> Result<Self, DatasetError> {
        Ok(Self {
            source_bytes: checked(self.source_bytes, other.source_bytes)?,
            host_bytes: checked(self.host_bytes, other.host_bytes)?,
            staging_bytes: checked(self.staging_bytes, other.staging_bytes)?,
            gpu_bytes: checked(self.gpu_bytes, other.gpu_bytes)?,
        })
    }
}

fn checked(left: u64, right: u64) -> Result<u64, DatasetError> {
    left.checked_add(right)
        .ok_or(DatasetError::FootprintOverflow)
}

#[cfg(test)]
#[path = "footprint_tests.rs"]
mod tests;
