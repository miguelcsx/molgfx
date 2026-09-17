//! Bounded two-level hierarchy over resident out-of-core pages.

mod index;
mod query;
mod types;

pub use index::PagedSpatialIndex;
pub use types::{
    SpatialCandidate, SpatialChunk, SpatialError, SpatialMaintenance, SpatialPageToken,
};

#[cfg(test)]
#[path = "spatial_tests.rs"]
mod tests;
