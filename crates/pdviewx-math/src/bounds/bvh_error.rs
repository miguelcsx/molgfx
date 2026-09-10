//! Typed limits of the compact GPU BVH layout.

use std::fmt;

/// A hierarchy offset cannot be represented by the GPU node layout.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct BvhBuildError {
    pub(super) resource: &'static str,
    pub(super) index: u64,
    pub(super) maximum: u32,
}

impl BvhBuildError {
    /// The table whose offset overflowed.
    #[must_use]
    pub const fn resource(self) -> &'static str {
        self.resource
    }

    /// The rejected offset or source row.
    #[must_use]
    pub const fn index(self) -> u64 {
        self.index
    }

    /// Largest value accepted by that GPU field.
    #[must_use]
    pub const fn maximum(self) -> u32 {
        self.maximum
    }
}

impl fmt::Display for BvhBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} index {} exceeds the GPU limit {}",
            self.resource, self.index, self.maximum
        )
    }
}

impl std::error::Error for BvhBuildError {}
