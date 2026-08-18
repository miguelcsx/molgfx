//! CPU-side records whose layout is shared with the renderer.

pub(crate) mod gpu_types;

pub use gpu_types::{
    AtomFlags, AtomGpu, BondGpu, DrawIndirectArgs, EntityId, EntityKind, EntityRef, InteractionGpu,
    ParticleMotionGpu, PrimitiveGpu, VolumeSegmentRef,
};
