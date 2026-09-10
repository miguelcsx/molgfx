//! Host-neutral contracts for paged datasets and shared structure assets.

mod asset;
mod brick;
mod catalog;
mod descriptor;
mod error;
mod footprint;
mod generic;
mod identity;
mod payload;
mod payload_bricks;
mod payload_geometry;
mod payload_rows;
mod picking;
mod picking_error;
mod provider_bridge;
mod residency;

pub use asset::{StructureAsset, StructureAssetPlacement};
pub use brick::{
    BrickAddress, BrickCatalog, BrickDescriptor, BrickId, BrickMetadata, BrickShape,
    BrickValueRange, DirtyGeneration,
};
pub use catalog::{CatalogChildren, CatalogRoots, DatasetCatalog};
pub use descriptor::{ChunkBounds, ChunkDescriptor, ChunkSpan, PayloadKind};
pub use error::DatasetError;
pub use footprint::ChunkFootprint;
pub use generic::{
    AttributeChunkPayload, ChunkDomainKind, ChunkDomainRef, ChunkEntityRef, ChunkSpatialKind,
    ChunkVisualBinding, ChunkVisualDescriptor, InstanceChunkPayload, PagedRelation,
    PagedSpatialAnchor, PointChunkPayload, RelationChunkPayload, TemplatePartChunkRef,
};
pub use identity::{ChunkId, ChunkOccurrenceId, DatasetId, LocalRow, LogicalRow};
pub use payload::{ChunkData, ChunkPayload};
pub use payload_bricks::{LabelBrickPayload, OccupancyBrickPayload, VolumeBrickPayload};
pub use payload_geometry::{MeshChunkPayload, ProxyChunkPayload};
pub use payload_rows::{
    PropertyChunkPayload, PropertyValues, ScalarChunkPayload, StructureChunkPayload,
    TrajectoryChunkPayload, TrajectoryFramesPayload,
};
pub use picking::{
    GlobalPickIdentity, GpuPickToken, PagedPickResolver, PickGeneration, PickPageDescriptor,
    PickPageTicket, PickReadback, ResidentPage,
};
pub use picking_error::PickingError;
pub use provider_bridge::{ProviderBridgeError, ProviderDatasetBridge};
pub use residency::{
    DeviceLossReport, Eviction, FailureReason, HostWorkingSet, HostWorkingSetError,
    ResidencyBudget, ResidencyClass, ResidencyDetail, ResidencyError, ResidencyKey,
    ResidencyMachine, ResidencyOutput, ResidencyPhase, ResidencyRequest, ResidencySnapshot,
    ResidencyTicket, StaleCompletion, Usage,
};

#[cfg(test)]
#[path = "provider_bridge_tests.rs"]
mod provider_bridge_tests;
