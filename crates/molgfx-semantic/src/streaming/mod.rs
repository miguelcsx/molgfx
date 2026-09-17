//! Caller-owned out-of-core scheduling and biological LOD clustering.

mod bricks;
mod lod_scene;
#[path = "streaming.rs"]
pub(crate) mod plan;
mod planner;
mod spatial;
#[cfg(test)]
mod test_support;

pub use bricks::{
    AtlasSlot, BrickPage, BrickSelection, BrickSelectionScratch, BrickWorkingSet,
    BrickWorkingSetError, ClipmapLevel, ClipmapSelector,
};
pub use lod_scene::LodScene;
pub use molgfx_core::{
    ChunkFootprint, ChunkId, DatasetId, DeviceLossReport, Eviction, FailureReason, HostWorkingSet,
    HostWorkingSetError, LogicalRow, ResidencyBudget, ResidencyClass, ResidencyError, ResidencyKey,
    ResidencyMachine, ResidencyOutput, ResidencyPhase, ResidencyRequest, ResidencySnapshot,
    ResidencyTicket, StaleCompletion, Usage,
};
pub use plan::{LodCluster, LodClusterKey, LodFrame, LodIndex};
pub use planner::{ChunkKey, ChunkRequest, StreamPlan, StreamPlanner, StreamingBudget};
pub use spatial::{
    PagedSpatialIndex, SpatialCandidate, SpatialChunk, SpatialError, SpatialMaintenance,
    SpatialPageToken,
};
