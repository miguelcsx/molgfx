//! GPU-resident scene state and its revision-diffed synchronization.

mod buffers;
mod interaction_table;
mod label_geometry;
mod label_pack;
mod label_table;
mod label_types;
mod layouts;
mod mesh_caps;
mod mesh_slot;
mod overlay_table;
mod primitive_draw;
mod primitive_table;
mod probe_offsets;
mod ribbon_slot;
mod scalar_overlay;
mod segmentation_lookup;
mod segmentation_slot;
mod segmentation_uniforms;
mod slot_types;
mod slots;
mod structure;
mod surface_slot;
mod sync;
mod trajectory_slot;
mod uniforms;
mod volume_slot;
mod volume_uniforms;

pub(crate) use primitive_draw::{
    FAMILY_BOX, FAMILY_ELLIPSOID, FAMILY_PARTICLE, FAMILY_POLYGON, PrimitiveDrawGroup,
};
pub(crate) use sync::GpuScene;
pub(crate) use uniforms::{FrameUniforms, TemporalFrame};
