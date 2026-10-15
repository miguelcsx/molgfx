//! Retained scratch for the per-frame draw path.

use crate::scene_gpu::VisibilityKey;
use crate::scene_gpu::surface_field::SurfaceFieldKey;

/// Scratch the record passes reuse so a steady-state frame allocates nothing.
#[derive(Debug, Default)]
pub(crate) struct DrawScratch {
    /// Dispatch owners, behind a lock because the record passes read the
    /// scene through a shared borrow.
    pub(crate) cull: std::sync::Mutex<CullScratch<VisibilityKey>>,
    /// Live surface-field keys, rebuilt each frame.
    pub(crate) surface_keys: Vec<SurfaceFieldKey>,
}

/// Marks which slots own the dispatch for their shared visibility and
/// indirect destination.
///
/// Slots that share both would otherwise cull the same lists twice. Finding
/// the first of each group is a sort over the keyed slots, so it costs
/// `O(S log S)` for `S` slots where a `contains` scan over the groups seen so
/// far cost `O(S²)`. Both vectors keep their capacity between calls, so a
/// steady-state frame allocates nothing.
#[derive(Debug)]
pub(crate) struct CullScratch<K> {
    keyed: Vec<(K, u64, usize)>,
    owner: Vec<bool>,
}

impl<K> Default for CullScratch<K> {
    fn default() -> Self {
        Self {
            keyed: Vec::new(),
            owner: Vec::new(),
        }
    }
}

impl<K: Copy + Ord> CullScratch<K> {
    /// Recomputes the owners from each slot's key, in slot order.
    ///
    /// `None` keys own their dispatch unconditionally.
    pub(crate) fn mark(&mut self, keys: impl Iterator<Item = Option<(K, u64)>>) {
        self.keyed.clear();
        self.owner.clear();
        for (slot, key) in keys.enumerate() {
            self.owner.push(key.is_none());
            if let Some((key, destination)) = key {
                self.keyed.push((key, destination, slot));
            }
        }
        self.keyed.sort_unstable();
        let mut previous: Option<(K, u64)> = None;
        for &(key, destination, slot) in &self.keyed {
            if previous != Some((key, destination))
                && let Some(owner) = self.owner.get_mut(slot)
            {
                *owner = true;
            }
            previous = Some((key, destination));
        }
    }

    /// Whether the slot at `index` owns its dispatch.
    pub(crate) fn owns(&self, index: usize) -> bool {
        self.owner.get(index).is_some_and(|owner| *owner)
    }
}

#[cfg(test)]
#[path = "cull_scratch_tests.rs"]
mod tests;
