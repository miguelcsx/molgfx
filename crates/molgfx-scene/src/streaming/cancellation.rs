//! Cooperative cancellation of an in-flight request.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Cooperative cancellation shared across one request batch.
#[derive(Clone, Debug, Default)]
pub struct Cancellation(pub(super) Arc<AtomicBool>);

impl Cancellation {
    /// Requests cancellation.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    /// Whether cancellation has been requested.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}
