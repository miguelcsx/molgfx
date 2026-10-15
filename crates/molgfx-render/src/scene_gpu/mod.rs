//! GPU-resident scene state and its revision-diffed synchronization.

mod arenas;
mod batches;
mod bead_recipes;
mod caches;
mod draw_family;
mod frame;
mod generic_visual;
mod interaction_table;
mod labels;
mod ligand_poses;
mod mesh_caps;
mod mesh_slot;
mod nucleotide_pairs;
mod overlays;
mod picking;
mod primitive_draw;
mod primitive_packing;
mod primitive_table;
mod relation_anchor;
mod ribbon_slot;
mod slot_state;
mod snfg_class;
mod surfaces;
mod sync;
mod visual;
mod visual_parameters;
mod visual_programs;
mod visual_properties;
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
use labels::label_table;
use labels::label_types;
use ligand_poses::ligand_pose_table;
use ligand_poses::ligand_pose_types;
pub(crate) use overlays::color_uniforms;
use overlays::overlay_table;
use overlays::scalar_overlay;
use picking::picking_pages;
use picking::segmentation_slot;
use slot_state::slot_types;
use slot_state::slots;
pub(crate) use slot_state::trajectory_slot;
use surfaces::field_boundary;
use surfaces::grid_fit;
use surfaces::probe_offsets;
use surfaces::surface_cache;
use surfaces::surface_field;
use surfaces::surface_slot;
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
