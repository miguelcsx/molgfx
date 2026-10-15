//! Bounded sparse-volume atlas with generational publication.
//!
//! Storage is `O(resident brick capacity)`. Upload, lookup and retirement do
//! not depend on the logical volume extent, and a stable frame performs no
//! allocation or queue write.

mod page_table;
mod scene;
pub(crate) mod types;
pub(crate) mod upload;

#[cfg(test)]
#[path = "brick_atlas_tests.rs"]
mod tests;
