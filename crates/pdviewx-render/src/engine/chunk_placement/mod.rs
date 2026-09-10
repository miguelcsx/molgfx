//! Declarative placement of globally identified resident chunks.

mod bonds;
mod errors;
mod instances;
mod points;
mod relations;
mod representation;
mod structures;
mod windows;

pub use bonds::BondChunkPlacement;
pub use errors::{ChunkPlacementError, ChunkPlacementStatus};
pub use instances::InstanceChunkPlacement;
pub use points::PointChunkPlacement;
pub use relations::RelationChunkPlacement;
pub use representation::ChunkRepresentation;
pub use structures::StructureChunkPlacement;
pub use windows::{AttributeChunkWindow, InstanceChunkWindow, TrajectoryChunkWindow};

/// Caller identity for one independently mutable spatial chunk occurrence.
pub use pdviewx_core::ChunkOccurrenceId as ChunkPlacementId;
