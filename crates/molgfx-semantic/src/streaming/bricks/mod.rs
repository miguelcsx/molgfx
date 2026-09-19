//! Fixed-capacity sparse-brick working-set and clipmap policies.

mod clipmap;
mod page_table;
mod types;

pub use clipmap::ClipmapSelector;
pub use page_table::BrickWorkingSet;
pub(crate) use types::ClipmapQueryStats;
pub use types::{
    AtlasSlot, BrickPage, BrickSelection, BrickSelectionScratch, BrickWorkingSetError, ClipmapLevel,
};

#[cfg(test)]
#[path = "bricks_tests.rs"]
mod tests;
