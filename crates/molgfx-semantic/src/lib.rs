//! Deterministic semantic policies for focus, property mapping and biological LOD.

#![forbid(unsafe_code)]

mod composition;
mod detail;
mod focus;
mod streaming;

pub use composition::{
    CompositionError, DifferenceCompositionStyle, DifferenceLayer, EnsembleCompositionStyle,
    EnsembleLayer, FocusCompositionStyle, FocusLayer, GenericCompositionScene,
    GenericCompositionView,
};
pub use detail::{LodLevel, LodPolicy, MappingError, PropertyMapping};
pub use focus::{SurfaceZone, SurfaceZoneScene, SurfaceZoneStyle};
pub use streaming::{
    AtlasSlot, BrickPage, BrickSelection, BrickSelectionScratch, BrickWorkingSet,
    BrickWorkingSetError, ChunkFootprint, ChunkId, ChunkKey, ChunkRequest, ClipmapLevel,
    ClipmapSelector, DatasetId, DeviceLossReport, Eviction, FailureReason, HostWorkingSet,
    HostWorkingSetError, LodCluster, LodClusterKey, LodFrame, LodIndex, LodScene, LogicalRow,
    PagedSpatialIndex, ResidencyBudget, ResidencyClass, ResidencyError, ResidencyKey,
    ResidencyMachine, ResidencyOutput, ResidencyPhase, ResidencyRequest, ResidencySnapshot,
    ResidencyTicket, SpatialCandidate, SpatialChunk, SpatialError, SpatialMaintenance,
    SpatialPageToken, StaleCompletion, StreamPlan, StreamPlanner, StreamingBudget, Usage,
};
