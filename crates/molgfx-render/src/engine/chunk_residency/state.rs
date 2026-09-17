//! Bounded GPU residency state and allocation initialization.

use super::{generic::TrackedGenericChunk, tracked::TrackedChunk, trajectory::TrackedFrame};
use crate::ResidencyConfig;
use crate::engine::bond_draw_plan::ResidentAtomPage;
use crate::engine::bond_residency::BondGpuResidency;
use crate::engine::chunk_draw_plan::{
    ChunkClusterGpu, ResidentAttributeMaterialization, ResidentChunkPlacement,
    ResidentInstanceChunkPlacement, ResidentRelationChunkPlacement, ResidentTrajectoryWindow,
};
use crate::engine::chunk_residency_support::{
    cluster_capacity, create_cluster_buffer, create_coordinate_buffer,
};
use crate::engine::{
    AttributeChunkWindow, ChunkPlacementId, ChunkResidencyError, InstanceChunkPlacement,
    InstanceChunkWindow, PointChunkPlacement, RelationChunkPlacement, StructureChunkPlacement,
    TrajectoryChunkWindow,
};
use crate::residency::LazyUploadRing;
use hashbrown::HashMap;
use molgfx_core::{PagedRelation, ResidencyKey, ResidencyTicket};
use molgfx_gpu::{ArenaAllocation, Device, PagedArena};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PhysicalBacking {
    Placeholder,
    Resident,
}

#[derive(Debug)]
pub(in crate::engine) struct ChunkGpuResidency<D: Device> {
    pub(super) config: ResidencyConfig,
    pub(super) arena: PagedArena,
    pub(super) frame_arena: PagedArena,
    pub(super) cluster_arena: PagedArena,
    pub(super) uploads: LazyUploadRing,
    pub(super) buffer: D::Buffer,
    pub(super) display_buffer: D::Buffer,
    pub(super) frame_buffer: D::Buffer,
    pub(super) cluster_buffer: D::Buffer,
    source_backing: PhysicalBacking,
    display_backing: PhysicalBacking,
    frame_backing: PhysicalBacking,
    cluster_backing: PhysicalBacking,
    pub(super) binding_revision: u64,
    pub(super) tracked: Vec<TrackedChunk>,
    pub(super) tracked_index: HashMap<(ResidencyKey, u64), usize>,
    pub(super) generic: Vec<TrackedGenericChunk>,
    pub(super) generic_index: HashMap<(ResidencyKey, u64), usize>,
    pub(super) frames: Vec<TrackedFrame>,
    pub(super) frame_index: HashMap<(ResidencyKey, u64), usize>,
    pub(super) completed: Vec<ResidencyTicket>,
    pub(super) placements: Vec<StructureChunkPlacement>,
    pub(super) placement_index: HashMap<ChunkPlacementId, usize>,
    pub(super) point_placements: Vec<PointChunkPlacement>,
    pub(super) point_placement_index: HashMap<ChunkPlacementId, usize>,
    pub(super) instance_placements: Vec<InstanceChunkPlacement>,
    pub(super) instance_placement_index: HashMap<ChunkPlacementId, usize>,
    pub(super) instance_windows: Vec<InstanceChunkWindow>,
    pub(super) attribute_windows: Vec<AttributeChunkWindow>,
    pub(super) attribute_materializations: Vec<ResidentAttributeMaterialization>,
    pub(super) attribute_materialization_allocations: Vec<ArenaAllocation>,
    pub(super) instance_plan: Vec<ResidentInstanceChunkPlacement>,
    pub(super) relation_placements: Vec<RelationChunkPlacement>,
    pub(super) relation_placement_index: HashMap<ChunkPlacementId, usize>,
    pub(super) relation_sources: HashMap<(ResidencyKey, u64), Arc<[PagedRelation]>>,
    pub(super) relation_plan: Vec<ResidentRelationChunkPlacement>,
    pub(super) trajectory_windows: Vec<TrajectoryChunkWindow>,
    pub(super) trajectory_plan: Vec<ResidentTrajectoryWindow>,
    pub(super) draw_plan: Vec<ResidentChunkPlacement>,
    pub(super) cluster_scratch: Vec<ChunkClusterGpu>,
    pub(super) radius_scratch: Vec<f32>,
    pub(super) atom_page_scratch: Vec<ResidentAtomPage>,
    pub(super) bonds: BondGpuResidency<D>,
    pub(super) revision: u64,
    pub(super) relation_revision: u64,
    pub(super) instance_timeline_revision: u64,
    pub(super) attribute_timeline_revision: u64,
    pub(super) planned_revision: u64,
}

impl<D: Device> ChunkGpuResidency<D> {
    pub(in crate::engine) fn new(
        device: &D,
        config: ResidencyConfig,
    ) -> Result<Self, ChunkResidencyError> {
        let arena = PagedArena::new(config.page_size, config.page_count)?;
        let frame_arena = PagedArena::new(config.page_size, config.page_count)?;
        let cluster_capacity = cluster_capacity(arena.capacity_bytes(), config.machine_capacity)?;
        let cluster_arena = PagedArena::new(
            std::mem::size_of::<ChunkClusterGpu>() as u64,
            cluster_capacity,
        )?;
        let uploads = LazyUploadRing::new(config.uploads)?;
        let buffer = create_coordinate_buffer(device, 256, "unused resident structure chunks")?;
        let display_buffer =
            create_coordinate_buffer(device, 256, "unused resident display coordinates")?;
        let frame_buffer =
            create_coordinate_buffer(device, 256, "unused resident trajectory frames")?;
        let cluster_buffer =
            create_coordinate_buffer(device, 256, "unused resident structure chunk clusters")?;
        let bonds = BondGpuResidency::new(device, config)?;
        Ok(Self {
            config,
            arena,
            frame_arena,
            cluster_arena,
            uploads,
            buffer,
            display_buffer,
            frame_buffer,
            cluster_buffer,
            source_backing: PhysicalBacking::Placeholder,
            display_backing: PhysicalBacking::Placeholder,
            frame_backing: PhysicalBacking::Placeholder,
            cluster_backing: PhysicalBacking::Placeholder,
            binding_revision: 1,
            tracked: Vec::with_capacity(config.machine_capacity),
            tracked_index: HashMap::with_capacity(config.machine_capacity),
            generic: Vec::with_capacity(config.machine_capacity),
            generic_index: HashMap::with_capacity(config.machine_capacity),
            frames: Vec::with_capacity(config.machine_capacity),
            frame_index: HashMap::with_capacity(config.machine_capacity),
            completed: Vec::with_capacity(config.machine_capacity),
            placements: Vec::with_capacity(config.machine_capacity),
            placement_index: HashMap::with_capacity(config.machine_capacity),
            point_placements: Vec::with_capacity(config.machine_capacity),
            point_placement_index: HashMap::with_capacity(config.machine_capacity),
            instance_placements: Vec::with_capacity(config.machine_capacity),
            instance_placement_index: HashMap::with_capacity(config.machine_capacity),
            instance_windows: Vec::with_capacity(config.machine_capacity),
            attribute_windows: Vec::with_capacity(config.machine_capacity),
            attribute_materializations: Vec::with_capacity(config.machine_capacity),
            attribute_materialization_allocations: Vec::with_capacity(config.machine_capacity),
            instance_plan: Vec::with_capacity(config.machine_capacity),
            relation_placements: Vec::with_capacity(config.machine_capacity),
            relation_placement_index: HashMap::with_capacity(config.machine_capacity),
            relation_sources: HashMap::with_capacity(config.machine_capacity),
            relation_plan: Vec::with_capacity(config.machine_capacity),
            trajectory_windows: Vec::with_capacity(config.machine_capacity),
            trajectory_plan: Vec::with_capacity(config.machine_capacity),
            draw_plan: Vec::with_capacity(config.machine_capacity),
            cluster_scratch: Vec::with_capacity(cluster_capacity as usize),
            radius_scratch: Vec::new(),
            atom_page_scratch: Vec::with_capacity(config.machine_capacity),
            bonds,
            revision: 1,
            relation_revision: 1,
            instance_timeline_revision: 1,
            attribute_timeline_revision: 1,
            planned_revision: 0,
        })
    }

    pub(super) fn ensure_source_buffer(&mut self, device: &D) -> Result<(), ChunkResidencyError> {
        if self.source_backing == PhysicalBacking::Placeholder {
            self.buffer = create_coordinate_buffer(
                device,
                self.arena.capacity_bytes(),
                "resident structure chunks",
            )?;
            self.source_backing = PhysicalBacking::Resident;
            self.bump_binding_revision();
        }
        Ok(())
    }

    pub(super) fn ensure_display_buffer(&mut self, device: &D) -> Result<(), ChunkResidencyError> {
        if self.display_backing == PhysicalBacking::Placeholder {
            self.display_buffer = create_coordinate_buffer(
                device,
                self.arena.capacity_bytes(),
                "resident display coordinates",
            )?;
            self.display_backing = PhysicalBacking::Resident;
            self.bump_binding_revision();
        }
        Ok(())
    }

    pub(super) fn ensure_frame_buffer(&mut self, device: &D) -> Result<(), ChunkResidencyError> {
        if self.frame_backing == PhysicalBacking::Placeholder {
            self.frame_buffer = create_coordinate_buffer(
                device,
                self.frame_arena.capacity_bytes(),
                "resident trajectory frames",
            )?;
            self.frame_backing = PhysicalBacking::Resident;
            self.bump_binding_revision();
        }
        Ok(())
    }

    pub(super) fn ensure_cluster_buffer(&mut self, device: &D) -> Result<(), ChunkResidencyError> {
        if self.cluster_backing == PhysicalBacking::Placeholder {
            self.cluster_buffer =
                create_cluster_buffer(device, self.cluster_arena.capacity_bytes())?;
            self.cluster_backing = PhysicalBacking::Resident;
            self.bump_binding_revision();
        }
        Ok(())
    }

    fn bump_binding_revision(&mut self) {
        self.binding_revision = self.binding_revision.wrapping_add(1).max(1);
    }
}
