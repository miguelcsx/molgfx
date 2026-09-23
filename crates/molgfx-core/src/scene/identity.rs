//! Process-local scene identity for renderer-cache invalidation.

use super::Scene;
use crate::DatasetId;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(1);

#[derive(Debug)]
pub(super) struct SceneIdentity(u64);

impl SceneIdentity {
    pub(super) const fn get(&self) -> u64 {
        self.0
    }
}

impl Default for SceneIdentity {
    fn default() -> Self {
        Self(NEXT.fetch_add(1, Ordering::Relaxed))
    }
}

impl Scene {
    /// Process-local identity used to invalidate renderer caches when one
    /// engine is reused with another scene carrying coincident revision values.
    #[doc(hidden)]
    #[must_use]
    pub const fn cache_identity(&self) -> u64 {
        self.identity.get()
    }

    /// A dataset identity no structure in this scene already uses.
    ///
    /// The identity is what tells two molecular assets apart downstream: the
    /// renderer keys its GPU resources on it and refuses two fingerprints under
    /// one identity. Handing every added structure the same legacy identity
    /// therefore made a second structure in one scene indistinguishable from a
    /// changed first one, and the renderer rejected the whole scene. The
    /// counter starts above the legacy value, which stays reserved for the
    /// single transitional constructor that has no caller-supplied identity.
    pub(super) fn allocate_dataset(&mut self) -> DatasetId {
        let dataset = DatasetId::new(self.next_dataset);
        self.next_dataset = self.next_dataset.saturating_add(1);
        dataset
    }

    /// Steps the allocator past an identity a caller supplied directly.
    ///
    /// A caller-supplied asset may carry any identity, so a later
    /// `add_structure` must not hand out the same one.
    pub(super) fn reserve_dataset(&mut self, dataset: DatasetId) {
        if dataset.get() >= self.next_dataset {
            self.next_dataset = dataset.get().saturating_add(1);
        }
    }
}
