//! Scene-independent generic payloads for the paged residency lifecycle.

mod anchors;
mod payloads;
mod visual;

pub use anchors::{
    ChunkDomainKind, ChunkDomainRef, ChunkEntityRef, ChunkSpatialKind, PagedRelation,
    PagedSpatialAnchor, TemplatePartChunkRef,
};
pub use payloads::{
    AttributeChunkPayload, InstanceChunkPayload, PointChunkPayload, RelationChunkPayload,
};
pub use visual::{ChunkVisualBinding, ChunkVisualDescriptor};

#[cfg(test)]
mod tests;
