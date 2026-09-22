//! GPU-resident scene state and its revision-diffed synchronization.

mod acceleration_cache;
mod asset;
mod asset_arena;
pub(crate) mod brick_atlas;
mod buffers;
pub(crate) mod color_uniforms;
mod dispatch;
mod draw_family;
mod generic_visual;
mod grow_buffer;
mod indirect_arena;
mod instance_batch_table;
mod interaction_table;
mod label_geometry;
mod label_pack;
mod label_table;
mod label_types;
mod layouts;
mod ligand_pose_plan;
mod ligand_pose_sampling;
mod ligand_pose_table;
mod ligand_pose_types;
mod ligand_pose_upload;
mod mesh_caps;
mod mesh_slot;
mod occupancy_slot;
mod overlay_table;
pub(crate) mod paged_bonds;
pub(crate) mod paged_chunks;
mod picking_pages;
mod placement_acceleration;
mod point_batch_table;
mod primitive_draw;
mod primitive_packing;
mod primitive_table;
mod probe_offsets;
mod quality_acceleration;
mod quality_hardware;
mod record_cache;
mod record_key;
mod record_pack;
mod relation_anchor;
mod ribbon_slot;
mod scalar_overlay;
mod segmentation_lookup;
mod segmentation_slot;
mod segmentation_uniforms;
mod slot_types;
mod slots;
mod structure;
mod surface_cache;
mod surface_components;
mod surface_field;
mod surface_slot;
mod sync;
pub(crate) use sync::{PagedInstancesSync, PagedRelationsSync, SceneSync};
mod trajectory_slot;
mod uniforms;
mod visibility_cache;
mod visual;
mod visual_parameters;
mod visual_programs;
mod visual_properties;
mod volume_slot;
mod volume_uniforms;

pub(crate) use asset_arena::AssetArenaError;
pub(crate) use draw_family::DrawFamily;
pub(crate) use indirect_arena::IndirectSlotKey;
pub(crate) use instance_batch_table::{GENERIC_INSTANCE_CAPSULE, GENERIC_INSTANCE_SPHERE};
pub(crate) use ligand_pose_types::{
    LigandPoseDrawGroup, POSE_CAPSULE, POSE_SPHERE, PoseTableStats,
};
pub(crate) use primitive_draw::{
    FAMILY_BOX, FAMILY_ELLIPSOID, FAMILY_PARTICLE, FAMILY_POLYGON, POLYGON_HEXAGON,
    POLYGON_PENTAGON, PrimitiveDrawGroup,
};
pub(crate) use record_cache::RecordKey;
pub(crate) use segmentation_slot::SegmentationPipelineKey;
pub(crate) use slot_types::SlotShading;
pub(crate) use surface_field::SurfaceFieldKey;
pub(crate) use sync::GpuScene;
pub(crate) use uniforms::{FrameUniforms, TemporalFrame};
pub(crate) use visibility_cache::VisibilityKey;
