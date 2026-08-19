//! Process-local scene identity for renderer-cache invalidation.

use super::Scene;
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
}
