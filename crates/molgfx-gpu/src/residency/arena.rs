//! Deterministic page-granular suballocation.
//!
//! Allocation is first-fit over address-sorted free runs. Allocation is
//! `O(free runs + allocated pages)` and release is
//! `O(free runs + released pages)`. All metadata is reserved at construction,
//! so neither operation grows the heap after warm-up.

use thiserror::Error;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FreeRun {
    start: u32,
    pages: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PageOwner {
    Free,
    Head { generation: u64, pages: u32 },
    Tail { head: u32, generation: u64 },
}

/// A generation-checked allocation within a [`PagedArena`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArenaAllocation {
    start_page: u32,
    page_count: u32,
    generation: u64,
    byte_offset: u64,
    byte_len: u64,
}

impl ArenaAllocation {
    /// First page occupied by this allocation.
    #[must_use]
    pub const fn start_page(self) -> u32 {
        self.start_page
    }

    /// Number of whole pages reserved.
    #[must_use]
    pub const fn page_count(self) -> u32 {
        self.page_count
    }

    /// Byte offset suitable for binding or copy commands.
    #[must_use]
    pub const fn byte_offset(self) -> u64 {
        self.byte_offset
    }

    /// Requested byte length, excluding page padding.
    #[must_use]
    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }
}

/// Observable arena counters. Values are cumulative except resident bytes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ArenaMetrics {
    /// Bytes currently covered by live pages.
    pub resident_bytes: u64,
    /// Highest live page footprint observed.
    pub peak_resident_bytes: u64,
    /// Sum of requested payload bytes accepted.
    pub requested_bytes: u64,
    /// Sum of page-rounded bytes accepted.
    pub allocated_bytes: u64,
    /// Number of successful allocations.
    pub allocations: u64,
    /// Number of requests rejected for capacity.
    pub allocation_stalls: u64,
    /// Payload bytes rejected for capacity.
    pub stalled_bytes: u64,
    /// Host storage allocations performed by this primitive.
    pub host_allocation_events: u64,
}

/// Invalid requests and stale allocation handles.
#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum ArenaError {
    /// Page size and page count must both be non-zero.
    #[error("page size and page count must both be non-zero")]
    InvalidConfiguration,
    /// Zero-byte allocations are not meaningful.
    #[error("an arena allocation must contain at least one byte")]
    EmptyAllocation,
    /// The requested byte count cannot be represented by this arena.
    #[error("arena allocation size overflow")]
    SizeOverflow,
    /// No contiguous free run can satisfy the request.
    #[error("no contiguous page run can satisfy {requested_bytes} bytes")]
    OutOfMemory {
        /// Payload size that could not be placed.
        requested_bytes: u64,
    },
    /// The handle was already freed or belongs to an older allocation.
    #[error("arena allocation is stale")]
    StaleAllocation,
    /// A fixed-capacity arena cannot be extended by zero pages.
    #[error("arena growth must add at least one page")]
    EmptyGrowth,
    /// The enlarged page count cannot be represented by the arena.
    #[error("arena page count overflow")]
    PageCountOverflow,
}

/// A fixed-capacity page allocator with deterministic first-fit placement.
#[derive(Debug)]
pub struct PagedArena {
    page_size: u64,
    page_count: u32,
    owners: Vec<PageOwner>,
    free_runs: Vec<FreeRun>,
    next_generation: u64,
    metrics: ArenaMetrics,
}

impl PagedArena {
    /// Reserves all allocator metadata up front.
    ///
    /// # Errors
    ///
    /// Returns [`ArenaError::InvalidConfiguration`] for an empty arena.
    pub fn new(page_size: u64, page_count: u32) -> Result<Self, ArenaError> {
        if page_size == 0 || page_count == 0 {
            return Err(ArenaError::InvalidConfiguration);
        }
        let owner_count = page_count as usize;
        let owners = vec![PageOwner::Free; owner_count];
        let mut free_runs = Vec::with_capacity(owner_count);
        free_runs.push(FreeRun {
            start: 0,
            pages: page_count,
        });
        Ok(Self {
            page_size,
            page_count,
            owners,
            free_runs,
            next_generation: 1,
            metrics: ArenaMetrics {
                host_allocation_events: 2,
                ..ArenaMetrics::default()
            },
        })
    }

    /// Reserves the first address-ordered run large enough for `byte_len`.
    ///
    /// # Errors
    ///
    /// Empty, overflowing and unsatisfied requests are returned as values.
    pub fn allocate(&mut self, byte_len: u64) -> Result<ArenaAllocation, ArenaError> {
        if byte_len == 0 {
            return Err(ArenaError::EmptyAllocation);
        }
        let rounded = byte_len
            .checked_add(self.page_size - 1)
            .ok_or(ArenaError::SizeOverflow)?;
        let page_count_u64 = rounded / self.page_size;
        let page_count = u32::try_from(page_count_u64).map_err(|_| ArenaError::SizeOverflow)?;
        let Some(run_index) = self
            .free_runs
            .iter()
            .position(|run| run.pages >= page_count)
        else {
            self.metrics.allocation_stalls = self.metrics.allocation_stalls.saturating_add(1);
            self.metrics.stalled_bytes = self.metrics.stalled_bytes.saturating_add(byte_len);
            return Err(ArenaError::OutOfMemory {
                requested_bytes: byte_len,
            });
        };
        let start_page = self.take_pages(run_index, page_count);
        let generation = self.next_generation;
        self.next_generation = self.next_generation.wrapping_add(1).max(1);
        self.mark_owned(start_page, page_count, generation);
        let allocated = u64::from(page_count) * self.page_size;
        self.metrics.resident_bytes = self.metrics.resident_bytes.saturating_add(allocated);
        self.metrics.peak_resident_bytes = self
            .metrics
            .peak_resident_bytes
            .max(self.metrics.resident_bytes);
        self.metrics.requested_bytes = self.metrics.requested_bytes.saturating_add(byte_len);
        self.metrics.allocated_bytes = self.metrics.allocated_bytes.saturating_add(allocated);
        self.metrics.allocations = self.metrics.allocations.saturating_add(1);
        Ok(ArenaAllocation {
            start_page,
            page_count,
            generation,
            byte_offset: u64::from(start_page) * self.page_size,
            byte_len,
        })
    }

    /// Releases a live allocation and coalesces adjacent free runs.
    ///
    /// # Errors
    ///
    /// Returns [`ArenaError::StaleAllocation`] when generation or extent no
    /// longer matches the arena.
    pub fn release(&mut self, allocation: ArenaAllocation) -> Result<(), ArenaError> {
        if !self.is_live(allocation) {
            return Err(ArenaError::StaleAllocation);
        }
        let start = allocation.start_page as usize;
        let end = start + allocation.page_count as usize;
        self.owners[start..end].fill(PageOwner::Free);
        self.insert_free_run(FreeRun {
            start: allocation.start_page,
            pages: allocation.page_count,
        });
        self.metrics.resident_bytes -= u64::from(allocation.page_count) * self.page_size;
        Ok(())
    }

    /// Appends free pages without moving any live allocation.
    ///
    /// Existing byte offsets remain stable, allowing a physical owner to copy
    /// the old address space verbatim into a larger GPU buffer.
    ///
    /// # Errors
    ///
    /// Zero growth and page-count overflow are rejected as typed values.
    pub fn grow(&mut self, additional_pages: u32) -> Result<(), ArenaError> {
        if additional_pages == 0 {
            return Err(ArenaError::EmptyGrowth);
        }
        let old_pages = self.page_count;
        self.page_count = old_pages
            .checked_add(additional_pages)
            .ok_or(ArenaError::PageCountOverflow)?;
        self.owners.reserve(additional_pages as usize);
        self.owners.extend(std::iter::repeat_n(
            PageOwner::Free,
            additional_pages as usize,
        ));
        self.metrics.host_allocation_events = self.metrics.host_allocation_events.saturating_add(1);
        self.insert_free_run(FreeRun {
            start: old_pages,
            pages: additional_pages,
        });
        Ok(())
    }

    /// Current cumulative counters.
    #[must_use]
    pub const fn metrics(&self) -> ArenaMetrics {
        self.metrics
    }

    /// Total bytes represented by the arena.
    #[must_use]
    pub fn capacity_bytes(&self) -> u64 {
        self.owners.len() as u64 * self.page_size
    }

    /// Number of addressable pages.
    #[must_use]
    pub fn page_count(&self) -> u32 {
        self.page_count
    }

    fn take_pages(&mut self, index: usize, pages: u32) -> u32 {
        let start = self.free_runs[index].start;
        if self.free_runs[index].pages == pages {
            self.free_runs.remove(index);
        } else {
            self.free_runs[index].start += pages;
            self.free_runs[index].pages -= pages;
        }
        start
    }

    fn mark_owned(&mut self, start: u32, pages: u32, generation: u64) {
        let first = start as usize;
        self.owners[first] = PageOwner::Head { generation, pages };
        let end = first + pages as usize;
        for owner in &mut self.owners[first + 1..end] {
            *owner = PageOwner::Tail {
                head: start,
                generation,
            };
        }
    }

    fn is_live(&self, allocation: ArenaAllocation) -> bool {
        let Some(owner) = self.owners.get(allocation.start_page as usize) else {
            return false;
        };
        matches!(
            owner,
            PageOwner::Head { generation, pages }
                if *generation == allocation.generation && *pages == allocation.page_count
        )
    }

    fn insert_free_run(&mut self, run: FreeRun) {
        let index = self
            .free_runs
            .partition_point(|candidate| candidate.start < run.start);
        self.free_runs.insert(index, run);
        let merge_index = index.saturating_sub(1);
        self.coalesce_from(merge_index);
    }

    fn coalesce_from(&mut self, index: usize) {
        let cursor = index;
        while cursor + 1 < self.free_runs.len() {
            let current = self.free_runs[cursor];
            let next = self.free_runs[cursor + 1];
            if current.start + current.pages != next.start {
                break;
            }
            self.free_runs[cursor].pages += next.pages;
            self.free_runs.remove(cursor + 1);
        }
    }
}

#[cfg(test)]
#[path = "arena_tests.rs"]
mod tests;
