//! Compact CPU plans lowered only when paged residency or placements change.

use super::ChunkRepresentation;
use molgfx_core::{
    AnalyticTemplate, AttributeKind, ChunkDomainRef, ChunkId, ChunkSpan, DatasetId, EntityKind,
    PagedRelation, RelationStyle, ResidencyTicket,
};
use molgfx_gpu::ArenaAllocation;
use molgfx_math::{Mat4, Rgba8, Vec3};
use std::sync::Arc;

/// Conservative local-space bound for one fixed-size row cluster.
#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct ChunkClusterGpu {
    /// XYZ center and enclosing radius.
    pub(super) center_radius: [f32; 4],
}

impl ChunkClusterGpu {
    pub(super) fn from_positions(positions: &[[f32; 3]]) -> Self {
        let mut min = [f32::INFINITY; 3];
        let mut max = [f32::NEG_INFINITY; 3];
        for position in positions {
            for axis in 0..3 {
                min[axis] = min[axis].min(position[axis]);
                max[axis] = max[axis].max(position[axis]);
            }
        }
        let center = [
            (min[0] + max[0]) * 0.5,
            (min[1] + max[1]) * 0.5,
            (min[2] + max[2]) * 0.5,
        ];
        let mut radius_squared = 0.0_f32;
        for position in positions {
            let delta = [
                position[0] - center[0],
                position[1] - center[1],
                position[2] - center[2],
            ];
            radius_squared = radius_squared
                .max(delta[0].mul_add(delta[0], delta[1].mul_add(delta[1], delta[2] * delta[2])));
        }
        Self {
            center_radius: [center[0], center[1], center[2], radius_squared.sqrt()],
        }
    }
}

/// Physical state needed to lower one declarative placement.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ResidentChunkRange {
    pub(crate) ticket: ResidencyTicket,
    pub(crate) coordinate_allocation: ArenaAllocation,
    pub(crate) cluster_allocation: ArenaAllocation,
    pub(crate) cluster_count: u32,
    pub(crate) radius_base: u32,
    pub(crate) max_radius: f32,
    pub(crate) span: ChunkSpan,
}

/// One placement whose exact ticket is currently resident.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ResidentChunkPlacement {
    pub(crate) model_to_world: Mat4,
    pub(crate) representation: ChunkRepresentation,
    pub(crate) entity_kind: EntityKind,
    pub(crate) range: ResidentChunkRange,
}

/// One resident rigid-transform stream paired with shared analytic geometry.
#[derive(Clone, Debug)]
pub(crate) struct ResidentInstanceChunkPlacement {
    pub(crate) id: molgfx_core::ChunkOccurrenceId,
    pub(crate) ticket: ResidencyTicket,
    pub(crate) byte_offset: u64,
    pub(crate) span: ChunkSpan,
    pub(crate) template: Arc<AnalyticTemplate>,
    pub(crate) color: Rgba8,
    pub(crate) timeline: Option<ResidentInstanceChunkWindow>,
}

impl PartialEq for ResidentInstanceChunkPlacement {
    fn eq(&self, other: &Self) -> bool {
        self.ticket == other.ticket
            && self.id == other.id
            && self.byte_offset == other.byte_offset
            && self.span == other.span
            && Arc::ptr_eq(&self.template, &other.template)
            && self.color == other.color
            && self.timeline == other.timeline
    }
}

/// Two exact resident transform streams sampled directly by GPU consumers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ResidentInstanceChunkWindow {
    pub(crate) start_byte_offset: u64,
    pub(crate) end_byte_offset: u64,
    pub(crate) interpolation: f32,
}

/// One exact relation generation whose every spatial dependency is resident.
#[derive(Clone, Debug)]
pub(crate) struct ResidentRelationChunkPlacement {
    pub(crate) id: molgfx_core::ChunkOccurrenceId,
    pub(crate) ticket: ResidencyTicket,
    pub(crate) span: ChunkSpan,
    pub(crate) relations: Arc<[PagedRelation]>,
    pub(crate) style: RelationStyle,
    pub(crate) visual: Option<Arc<molgfx_core::ChunkVisualDescriptor>>,
}

/// One native-width resident attribute column in the shared generic arena.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ResidentAttributeColumn {
    pub(crate) ticket: ResidencyTicket,
    pub(crate) target: ChunkDomainRef,
    pub(crate) byte_offset: u64,
    pub(crate) byte_len: u64,
    pub(crate) local_rows: u32,
    pub(crate) kind: AttributeKind,
    pub(crate) timeline: Option<ResidentAttributeChunkWindow>,
}

/// Two exact resident column pages sampled directly by a visual evaluator.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ResidentAttributeChunkWindow {
    pub(crate) start_byte_offset: u64,
    pub(crate) end_byte_offset: u64,
    pub(crate) interpolation: f32,
    pub(crate) materialized_byte_offset: Option<u64>,
}

/// One optional output range in the shared paged `array<u32>` arena.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ResidentAttributeMaterialization {
    pub(crate) ticket: ResidencyTicket,
    pub(crate) start_byte_offset: u64,
    pub(crate) end_byte_offset: u64,
    pub(crate) output_byte_offset: u64,
    pub(crate) word_count: u32,
    pub(crate) interpolation: f32,
}

impl PartialEq for ResidentRelationChunkPlacement {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.ticket == other.ticket
            && self.span == other.span
            && Arc::ptr_eq(&self.relations, &other.relations)
            && self.style == other.style
            && match (&self.visual, &other.visual) {
                (Some(left), Some(right)) => Arc::ptr_eq(left, right),
                (None, None) => true,
                _ => false,
            }
    }
}

/// Branch-free physical source selected while a relation plan is rebuilt.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum ResidentSpatialAnchor {
    /// Immutable world-space endpoint.
    World(Vec3),
    /// One local position transformed by the selected occurrence.
    Position {
        source: ResidentPositionSource,
        byte_offset: u64,
        byte_len: u64,
        local_row: u32,
        model_to_world: Mat4,
    },
    /// One rigid transform, optionally carrying a template-local part center.
    Rigid {
        byte_offset: u64,
        byte_len: u64,
        timeline_start: u64,
        timeline_end: u64,
        interpolation: f32,
        local_row: u32,
        local_position: Vec3,
    },
}

/// Physical arena containing one resolved position endpoint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ResidentPositionSource {
    /// Provider atom coordinates after trajectory interpolation.
    Display,
    /// Generic point positions in the shared source arena.
    Generic,
}

/// One fully resident two-frame window lowered for GPU interpolation.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ResidentTrajectoryWindow {
    pub(crate) structure: ResidentChunkRange,
    pub(crate) start: crate::engine::ResidentTrajectoryChunk,
    pub(crate) end: crate::engine::ResidentTrajectoryChunk,
    pub(crate) interpolation: f32,
}

impl ResidentChunkPlacement {
    pub(crate) const fn dataset(self) -> DatasetId {
        self.range.ticket.key.dataset
    }

    pub(crate) const fn chunk(self) -> ChunkId {
        self.range.ticket.key.chunk
    }

    pub(crate) const fn span(self) -> ChunkSpan {
        self.range.span
    }

    pub(crate) const fn representation(self) -> ChunkRepresentation {
        self.representation
    }

    pub(crate) const fn entity_kind(self) -> EntityKind {
        self.entity_kind
    }
}
