//! Resident-page ownership and incremental hierarchy maintenance.
//!
//! BLAS work is `O(primitives in changed pages)`. TLAS rebuild/refit is
//! `O(resident pages log resident pages)`/`O(resident pages)` respectively;
//! neither operation receives or enumerates the logical catalog.

use super::types::{SpatialChunk, SpatialError, SpatialMaintenance, SpatialPageToken};
use molgfx_math::{Aabb, Bvh, BvhBuildScratch};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(super) enum TlasChange {
    #[default]
    Clean,
    Refit,
    Rebuild,
}

#[derive(Debug, Default)]
pub(super) struct ResidentBlas {
    pub(super) generation: u64,
    pub(super) chunk: Option<SpatialChunk>,
    pub(super) hierarchy: Bvh,
    build_scratch: BvhBuildScratch,
}

#[derive(Debug, Default)]
pub(super) struct QueryScratch {
    pub(super) tlas_walk: Vec<u32>,
    pub(super) pages: Vec<u32>,
    pub(super) blas_walk: Vec<u32>,
    pub(super) primitives: Vec<u32>,
}

/// TLAS over bounded resident chunks with one chunk-local BLAS per page.
///
/// The object owns no logical descriptors. Its memory is bounded by resident
/// capacity plus geometry in resident BLASes, making it suitable for datasets
/// whose descriptor space is much larger than addressable memory.
#[derive(Debug)]
pub struct PagedSpatialIndex {
    pub(super) pages: Vec<ResidentBlas>,
    pub(super) tlas_bounds: Vec<Aabb>,
    pub(super) tlas: Bvh,
    tlas_scratch: BvhBuildScratch,
    pub(super) query_scratch: QueryScratch,
    change: TlasChange,
    resident: u32,
}

impl PagedSpatialIndex {
    /// Allocates fixed page tables for a bounded resident working set.
    ///
    /// # Errors
    ///
    /// Returns [`SpatialError::CapacityTooLarge`] when page indices cannot be
    /// represented by the GPU-compatible `u32` namespace.
    pub fn new(capacity: usize) -> Result<Self, SpatialError> {
        let Ok(capacity_u32) = u32::try_from(capacity) else {
            return Err(SpatialError::CapacityTooLarge { capacity });
        };
        let mut pages = Vec::with_capacity(capacity);
        pages.resize_with(capacity, ResidentBlas::default);
        Ok(Self {
            pages,
            tlas_bounds: vec![Aabb::EMPTY; capacity],
            tlas: Bvh::default(),
            tlas_scratch: BvhBuildScratch::default(),
            query_scratch: QueryScratch::default(),
            change: TlasChange::Clean,
            resident: capacity_u32.saturating_sub(capacity_u32),
        })
    }

    /// Maximum resident pages; memory never follows logical chunk count.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.pages.len()
    }

    /// Number of pages currently represented by the next committed TLAS.
    #[must_use]
    pub const fn resident_len(&self) -> u32 {
        self.resident
    }

    /// Builds or replaces one resident BLAS using chunk-local primitive rows.
    /// Existing vector allocations in the reused page are retained.
    ///
    /// # Errors
    ///
    /// Returns a typed capacity, generation, bounds or hierarchy error.
    pub fn replace_page(
        &mut self,
        page: u32,
        chunk: SpatialChunk,
        primitive_bounds: &[Aabb],
    ) -> Result<SpatialPageToken, SpatialError> {
        validate_bounds(chunk, primitive_bounds)?;
        let capacity = self.capacity_u32();
        let Some(slot) = self.pages.get_mut(page as usize) else {
            return Err(SpatialError::PageOutsideCapacity { page, capacity });
        };
        let Some(generation) = slot.generation.checked_add(1) else {
            return Err(SpatialError::GenerationExhausted { page });
        };
        let was_resident = slot.chunk.is_some();
        slot.hierarchy
            .rebuild(primitive_bounds, &mut slot.build_scratch)?;
        slot.generation = generation;
        slot.chunk = Some(chunk);
        self.tlas_bounds[page as usize] = slot.hierarchy.bounds();
        if !was_resident {
            self.resident += 1;
            self.change = TlasChange::Rebuild;
        } else if self.change == TlasChange::Clean {
            self.change = TlasChange::Refit;
        }
        Ok(SpatialPageToken::new(page, generation))
    }

    /// Refits one moving resident BLAS without changing its topology.
    ///
    /// # Errors
    ///
    /// Returns a typed stale-token, bounds or hierarchy error.
    pub fn refit_page(
        &mut self,
        token: SpatialPageToken,
        primitive_bounds: &[Aabb],
    ) -> Result<(), SpatialError> {
        let chunk = self.chunk_for(token)?;
        validate_bounds(chunk, primitive_bounds)?;
        let Some(slot) = self.pages.get_mut(token.page() as usize) else {
            return Err(Self::stale(token));
        };
        slot.hierarchy.refit(primitive_bounds)?;
        self.tlas_bounds[token.page() as usize] = slot.hierarchy.bounds();
        if self.change == TlasChange::Clean {
            self.change = TlasChange::Refit;
        }
        Ok(())
    }

    /// Evicts one page and invalidates its old token.
    ///
    /// # Errors
    ///
    /// Returns [`SpatialError::StalePage`] when the token is no longer current.
    pub fn evict(&mut self, token: SpatialPageToken) -> Result<(), SpatialError> {
        self.chunk_for(token)?;
        let Some(slot) = self.pages.get_mut(token.page() as usize) else {
            return Err(Self::stale(token));
        };
        slot.chunk = None;
        self.tlas_bounds[token.page() as usize] = Aabb::EMPTY;
        self.resident -= 1;
        self.change = TlasChange::Rebuild;
        Ok(())
    }

    /// Applies staged membership or bound changes before shared traversal.
    ///
    /// # Errors
    ///
    /// Returns a typed hierarchy error if rebuild or refit cannot be represented.
    pub fn commit(&mut self) -> Result<SpatialMaintenance, SpatialError> {
        let rebuilt = self.change == TlasChange::Rebuild;
        match self.change {
            TlasChange::Clean => {}
            TlasChange::Refit => self.tlas.refit(&self.tlas_bounds)?,
            TlasChange::Rebuild => self
                .tlas
                .rebuild(&self.tlas_bounds, &mut self.tlas_scratch)?,
        }
        self.change = TlasChange::Clean;
        Ok(SpatialMaintenance {
            resident_bounds_visited: self.resident,
            rebuilt,
        })
    }

    pub(super) fn ensure_committed(&self) -> Result<(), SpatialError> {
        if self.change == TlasChange::Clean {
            Ok(())
        } else {
            Err(SpatialError::UncommittedChanges)
        }
    }

    fn chunk_for(&self, token: SpatialPageToken) -> Result<SpatialChunk, SpatialError> {
        let Some(slot) = self.pages.get(token.page() as usize) else {
            return Err(Self::stale(token));
        };
        if slot.generation != token.generation() {
            return Err(Self::stale(token));
        }
        match slot.chunk {
            Some(chunk) => Ok(chunk),
            None => Err(Self::stale(token)),
        }
    }

    fn capacity_u32(&self) -> u32 {
        match u32::try_from(self.pages.len()) {
            Ok(capacity) => capacity,
            Err(_) => u32::MAX,
        }
    }

    fn stale(token: SpatialPageToken) -> SpatialError {
        SpatialError::StalePage {
            page: token.page(),
            generation: token.generation(),
        }
    }
}

fn validate_bounds(chunk: SpatialChunk, bounds: &[Aabb]) -> Result<(), SpatialError> {
    if bounds.len() != chunk.rows.row_count() as usize {
        return Err(SpatialError::PrimitiveCountMismatch {
            chunk: chunk.chunk,
            expected: chunk.rows.row_count(),
            actual: bounds.len(),
        });
    }
    for (local, bound) in bounds.iter().enumerate() {
        if bound.is_empty() || !bound.min.is_finite() || !bound.max.is_finite() {
            let Ok(local) = u32::try_from(local) else {
                return Err(SpatialError::PrimitiveCountMismatch {
                    chunk: chunk.chunk,
                    expected: chunk.rows.row_count(),
                    actual: bounds.len(),
                });
            };
            return Err(SpatialError::InvalidPrimitiveBounds {
                chunk: chunk.chunk,
                local,
            });
        }
    }
    Ok(())
}
