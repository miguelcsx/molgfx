//! Compact sorted ledger and deterministic eviction policy.

use super::model::{
    DerivedCacheBudget, DerivedCacheClass, DerivedCacheKey, DerivedCacheUsage, DerivedFootprint,
    Entry, MaterializationPlan,
};

/// Cold-path ledger. Entries remain sorted by stable key, giving `O(log n)`
/// lookup without hash-table capacity or randomized iteration state. Pressure
/// scans are `O(n)` and occur only when a new optional allocation exceeds its
/// budget. Visible source data and required indirect state never enter it.
#[derive(Debug)]
pub(crate) struct DerivedCache {
    budget: DerivedCacheBudget,
    usage: DerivedCacheUsage,
    entries: Vec<Entry>,
    evicted: Vec<DerivedCacheKey>,
}

impl DerivedCache {
    pub(crate) const fn new(budget: DerivedCacheBudget) -> Self {
        Self {
            budget,
            usage: DerivedCacheUsage {
                cpu_bytes: 0,
                gpu_bytes: 0,
                peak_cpu_bytes: 0,
                peak_gpu_bytes: 0,
            },
            entries: Vec::new(),
            evicted: Vec::new(),
        }
    }

    pub(crate) const fn usage(&self) -> DerivedCacheUsage {
        self.usage
    }

    /// Replaces the hard limits and immediately releases what no longer fits.
    pub(crate) fn set_budget(&mut self, budget: DerivedCacheBudget) {
        self.budget = budget;
        self.trim();
    }

    pub(crate) fn retain<K: Into<DerivedCacheKey>>(
        &mut self,
        key: K,
        class: DerivedCacheClass,
        footprint: DerivedFootprint,
        frame: u64,
    ) -> bool {
        let key = key.into();
        if footprint.cpu_bytes > self.budget.cpu_bytes
            || footprint.gpu_bytes > self.budget.gpu_bytes
        {
            return false;
        }
        if let Ok(index) = self.index(key) {
            if self.entries[index].class == class && self.entries[index].footprint == footprint {
                self.entries[index].last_used_frame = frame;
                return true;
            }
            self.remove_index(index);
        }
        while self.would_exceed(footprint) {
            let Some(index) = self.eviction_index(frame) else {
                return false;
            };
            self.remove_index(index);
        }
        let index = match self.index(key) {
            Ok(index) | Err(index) => index,
        };
        self.entries.insert(
            index,
            Entry {
                key,
                class,
                footprint,
                last_used_frame: frame,
            },
        );
        self.usage.cpu_bytes += footprint.cpu_bytes;
        self.usage.gpu_bytes += footprint.gpu_bytes;
        self.usage.peak_cpu_bytes = self.usage.peak_cpu_bytes.max(self.usage.cpu_bytes);
        self.usage.peak_gpu_bytes = self.usage.peak_gpu_bytes.max(self.usage.gpu_bytes);
        true
    }

    pub(crate) fn plan<K: Into<DerivedCacheKey>>(
        &mut self,
        key: K,
        footprint: DerivedFootprint,
        consumers: u32,
        frame: u64,
    ) -> MaterializationPlan {
        let key = key.into();
        if consumers < 2
            || !self.retain(
                key,
                DerivedCacheClass::TimelineMaterialization,
                footprint,
                frame,
            )
        {
            MaterializationPlan::Direct
        } else {
            MaterializationPlan::Materialized
        }
    }

    pub(crate) fn release(&mut self, key: DerivedCacheKey) {
        if let Ok(index) = self.index(key) {
            self.remove_index(index);
        }
    }

    pub(crate) fn contains(&self, key: DerivedCacheKey) -> bool {
        self.index(key).is_ok()
    }

    /// Releases entries until the retained set fits the current budget.
    ///
    /// The budget is the only thing that destroys a derived resource: a
    /// visibility gate suppresses a draw, it never frees memory. Lowering the
    /// budget therefore has to be able to evict something already resident,
    /// which is what this walk is for. Entries leave in the same deterministic
    /// `(class, last_used_frame, key)` order as pressure eviction.
    pub(crate) fn trim(&mut self) {
        while self.would_exceed(DerivedFootprint {
            cpu_bytes: 0,
            gpu_bytes: 0,
        }) {
            let Some(index) = self
                .entries
                .iter()
                .enumerate()
                .min_by_key(|(_, entry)| (entry.class, entry.last_used_frame, entry.key))
                .map(|(index, _)| index)
            else {
                return;
            };
            self.remove_index(index);
        }
    }

    pub(crate) fn take_evictions(&mut self) -> impl Iterator<Item = DerivedCacheKey> + '_ {
        self.evicted.drain(..)
    }

    fn index(&self, key: DerivedCacheKey) -> Result<usize, usize> {
        self.entries.binary_search_by_key(&key, |entry| entry.key)
    }

    fn would_exceed(&self, footprint: DerivedFootprint) -> bool {
        self.usage.cpu_bytes.saturating_add(footprint.cpu_bytes) > self.budget.cpu_bytes
            || self.usage.gpu_bytes.saturating_add(footprint.gpu_bytes) > self.budget.gpu_bytes
    }

    fn eviction_index(&self, frame: u64) -> Option<usize> {
        self.entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.last_used_frame < frame)
            .min_by_key(|(_, entry)| (entry.class, entry.last_used_frame, entry.key))
            .map(|(index, _)| index)
    }

    fn remove_index(&mut self, index: usize) {
        let entry = self.entries.remove(index);
        self.usage.cpu_bytes -= entry.footprint.cpu_bytes;
        self.usage.gpu_bytes -= entry.footprint.gpu_bytes;
        self.evicted.push(entry.key);
    }
}
