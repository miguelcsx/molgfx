//! GPU-resident scene state and its revision-diffed synchronization.

mod arenas;
mod batches;
mod caches;
mod frame;
mod interactions;
mod labels;
mod ligand_poses;
mod meshes;
mod overlays;
mod picking;
mod primitives;
mod slot_state;
mod surfaces;
mod sync;
mod visuals;
mod volumes;

use arenas::asset;
use arenas::asset_arena;
use arenas::buffers;
use arenas::cull_scratch;
use arenas::dispatch;
pub(crate) use arenas::exposure_arena;
use arenas::grow_buffer;
use arenas::indexed_draw;
use arenas::indirect_arena;
pub(crate) use arenas::ramp_lut;
use batches::instance_batch_table;
pub(crate) use batches::paged_bonds;
pub(crate) use batches::paged_chunks;
use batches::point_batch_table;
use caches::acceleration_cache;
use caches::placement_acceleration;
use caches::quality_acceleration;
use caches::record_cache;
use caches::visibility_cache;
pub(crate) use frame::detail;
use frame::layouts;
use frame::structure;
use frame::uniforms;
use interactions::bead_recipes;
use interactions::interaction_table;
use interactions::nucleotide_pairs;
use interactions::snfg_class;
use labels::label_table;
use labels::label_types;
use ligand_poses::ligand_pose_table;
use ligand_poses::ligand_pose_types;
use meshes::generic_visual;
use meshes::mesh_slot;
use meshes::ribbon_slot;
pub(crate) use overlays::color_uniforms;
use overlays::overlay_table;
use overlays::scalar_overlay;
use picking::picking_pages;
use picking::segmentation_slot;
use primitives::draw_family;
use primitives::primitive_draw;
use primitives::primitive_packing;
use primitives::primitive_table;
use slot_state::slot_types;
use slot_state::slots;
pub(crate) use slot_state::trajectory_slot;
use surfaces::field_boundary;
use surfaces::grid_fit;
use surfaces::probe_offsets;
use surfaces::surface_cache;
use surfaces::surface_field;
use surfaces::surface_slot;
use visuals::visual;
use visuals::visual_parameters;
use visuals::visual_programs;
use visuals::visual_properties;
pub(crate) use volumes::brick_atlas;
use volumes::volume_boundary;
use volumes::volume_slot;
use volumes::volume_uniforms;

pub(crate) use sync::{PagedInstancesSync, PagedRelationsSync, SceneSync};

pub(crate) use asset_arena::AssetArenaError;
pub(crate) use draw_family::DrawFamily;
pub(crate) use field_boundary::{FieldGeometry, GeometryKind};
pub(crate) use indirect_arena::IndirectSlotKey;
pub(crate) use instance_batch_table::{GENERIC_INSTANCE_CAPSULE, GENERIC_INSTANCE_SPHERE};
pub(crate) use ligand_pose_types::{
    LigandPoseDrawGroup, POSE_CAPSULE, POSE_SPHERE, PoseTableStats,
};
pub(crate) use primitive_draw::{
    FAMILY_BOX, FAMILY_ELLIPSOID, FAMILY_PARTICLE, FAMILY_POLYGON, POLYGON_DIAMOND,
    POLYGON_HEXAGON, POLYGON_PENTAGON, POLYGON_STAR, POLYGON_TRIANGLE, PrimitiveDrawGroup,
};
pub(crate) use record_cache::RecordKey;
pub(crate) use segmentation_slot::SegmentationPipelineKey;
pub(crate) use slot_types::SlotShading;
pub(crate) use surface_field::SurfaceFieldKey;
pub(crate) use sync::GpuScene;
pub(crate) use uniforms::{FrameUniforms, TemporalFrame};
pub(crate) use visibility_cache::VisibilityKey;
