//! Allocation-reusing caller-owned out-of-core scheduling.
//!
//! Sorting is `O(requests log requests)` and residency membership is expected
//! `O(1)` per request. The planner retains its ordering and membership scratch;
//! callers that reuse a [`StreamPlan`] also avoid output allocation after its
//! capacities reach their high-water marks.

use super::plan::LodClusterKey;
use crate::LodLevel;
use molgfx_core::StructureHandle;
use std::collections::HashSet;

#[cfg(test)]
#[path = "planner_tests.rs"]
mod tests;

/// One caller-owned chunk request produced by the streaming planner.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ChunkRequest {
    /// Requested biological cluster.
    pub key: ChunkKey,
    /// Higher values are retained first under pressure.
    pub priority: f32,
    /// Resident byte estimate supplied by the caller's manifest.
    pub bytes: u64,
}

/// Stable stream key. It contains no file path and performs no I/O.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct ChunkKey {
    /// Owning structure.
    pub structure: StructureHandle,
    /// Detail level.
    pub level: LodLevel,
    /// Cluster row.
    pub index: u32,
}

impl From<LodClusterKey> for ChunkKey {
    fn from(key: LodClusterKey) -> Self {
        Self {
            structure: key.structure,
            level: key.level,
            index: key.index,
        }
    }
}

/// Hard cap for caller-provided resident chunks.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StreamingBudget {
    /// Maximum resident bytes.
    pub max_resident_bytes: u64,
    /// Maximum number of new requests emitted in one frame.
    pub max_requests_per_frame: usize,
}

impl Default for StreamingBudget {
    fn default() -> Self {
        Self {
            max_resident_bytes: 256 * 1024 * 1024,
            max_requests_per_frame: 64,
        }
    }
}

/// The result of one deterministic residency decision.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct StreamPlan {
    /// Chunks the caller should load or keep hot.
    pub retain: Vec<ChunkRequest>,
    /// Previously resident chunks that can be evicted.
    pub evict: Vec<ChunkKey>,
    /// Bytes retained after the decision.
    pub resident_bytes: u64,
}

/// Stable residency planner with persistent intermediate scratch.
#[derive(Debug)]
pub struct StreamPlanner {
    budget: StreamingBudget,
    resident_keys: HashSet<ChunkKey>,
    ordered: Vec<ChunkRequest>,
    selected_keys: HashSet<ChunkKey>,
}

impl Clone for StreamPlanner {
    fn clone(&self) -> Self {
        Self {
            budget: self.budget,
            resident_keys: self.resident_keys.clone(),
            ordered: Vec::new(),
            selected_keys: HashSet::new(),
        }
    }
}

impl StreamPlanner {
    /// Creates a planner with a bounded resident cache.
    #[must_use]
    pub fn new(budget: StreamingBudget) -> Self {
        Self {
            budget,
            resident_keys: HashSet::new(),
            ordered: Vec::new(),
            selected_keys: HashSet::new(),
        }
    }

    /// Current budget.
    #[must_use]
    pub const fn budget(&self) -> StreamingBudget {
        self.budget
    }

    /// Reconciles visible requests into reusable caller storage.
    ///
    /// The planner reuses its sorting and membership tables, while `output`
    /// retains its `retain` and `evict` capacities across calls.
    pub fn plan_into(&mut self, requests: &[ChunkRequest], output: &mut StreamPlan) {
        self.ordered.clear();
        self.ordered.extend_from_slice(requests);
        self.ordered
            .retain(|request| request.bytes > 0 && request.priority.is_finite());
        self.ordered.sort_unstable_by(|left, right| {
            right
                .priority
                .total_cmp(&left.priority)
                .then_with(|| left.key.cmp(&right.key))
        });

        self.selected_keys.clear();
        output.retain.clear();
        output.evict.clear();
        let mut bytes = 0u64;
        let mut new_requests = 0usize;
        for request in self.ordered.iter().copied() {
            if self.selected_keys.contains(&request.key)
                || bytes.saturating_add(request.bytes) > self.budget.max_resident_bytes
            {
                continue;
            }
            if !self.resident_keys.contains(&request.key) {
                if new_requests >= self.budget.max_requests_per_frame {
                    continue;
                }
                new_requests += 1;
            }
            self.selected_keys.insert(request.key);
            bytes = bytes.saturating_add(request.bytes);
            output.retain.push(request);
        }
        output
            .evict
            .extend(self.resident_keys.difference(&self.selected_keys).copied());
        output.evict.sort_unstable();
        output.resident_bytes = bytes;
        self.resident_keys.clear();
        self.resident_keys
            .extend(self.selected_keys.iter().copied());
    }

    #[cfg(test)]
    pub(super) fn scratch_capacities(&self) -> (usize, usize, usize) {
        (
            self.ordered.capacity(),
            self.resident_keys.capacity(),
            self.selected_keys.capacity(),
        )
    }
}
