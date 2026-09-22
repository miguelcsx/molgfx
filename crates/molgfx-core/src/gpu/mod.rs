//! CPU-side records whose layout is shared with the renderer.

pub(crate) mod gpu_types;
pub(crate) mod semantic_tag;

pub use gpu_types::{
    AtomFlags, AtomGpu, BondGpu, DrawIndirectArgs, EntityId, EntityIdError, EntityKind, EntityRef,
    InteractionGpu, ParticleMotionGpu, PrimitiveGpu, VolumeSegmentRef,
};
pub use semantic_tag::SemanticTag;
