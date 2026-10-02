//! Bounds on concurrent streaming work.

/// Hard scheduler bounds independent of provider behavior.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Limits {
    /// Maximum chunk identities in one provider call.
    pub maximum_batch: usize,
    /// Maximum concurrently active provider calls.
    pub maximum_in_flight: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            maximum_batch: 16,
            maximum_in_flight: 4,
        }
    }
}
