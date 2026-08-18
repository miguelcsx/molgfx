//! Deterministic semantic policies for focus, property mapping and biological LOD.

#![forbid(unsafe_code)]

mod composition;
mod detail;
mod focus;
mod streaming;

pub use composition::{
    AtomCorrespondence, DifferenceScene, DifferenceStyle, DifferenceView, EnsembleScene,
    EnsembleStyle, EnsembleView, ProbabilityCloudView,
};
pub use detail::{LodLevel, LodPolicy, MappingError, PropertyMapping};
pub use focus::{
    DistanceBands, FocusBand, FocusContext, FocusError, FocusScene, FocusStyle, FocusSurfaceExtent,
    FocusView, SurfaceZone, SurfaceZoneScene, SurfaceZoneStyle,
};
pub use streaming::{
    ChunkKey, ChunkRequest, LodCluster, LodClusterKey, LodFrame, LodIndex, LodScene, StreamPlan,
    StreamPlanner, StreamingBudget,
};
