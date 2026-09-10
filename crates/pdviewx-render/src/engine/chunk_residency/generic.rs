//! Generic payload uploads sharing the provider residency arena and fences.

#[path = "generic/relations.rs"]
pub(super) mod relations;
#[path = "generic/spatial_anchors.rs"]
mod spatial_anchors;
#[path = "generic/upload.rs"]
mod upload;

use super::{ChunkGpuResidency, ticket_key};
use crate::engine::chunk_draw_plan::{
    ChunkClusterGpu, ResidentChunkRange, ResidentInstanceChunkPlacement,
};
use crate::engine::{ChunkResidencyError, ResidentGenericChunk};
use pdviewx_core::{
    AttributeKind, ChunkDomainRef, ChunkPayload, ChunkSpan, PayloadKind, ResidencyKey,
    ResidencyTicket,
};
use pdviewx_gpu::{ArenaAllocation, Device, FenceValue};
use relations::RelationUploadLayout;

#[derive(Clone, Copy, Debug)]
pub(super) enum TrackedGenericChunk {
    Uploading {
        ticket: ResidencyTicket,
        allocation: ArenaAllocation,
        cluster_allocation: Option<ArenaAllocation>,
        cluster_count: u32,
        fence: FenceValue,
        kind: PayloadKind,
        target: Option<ChunkDomainRef>,
        span: ChunkSpan,
        byte_len: u64,
        stride: u32,
        attribute_kind: Option<AttributeKind>,
        relation_layout: Option<RelationUploadLayout>,
        cancelled: bool,
    },
    Resident {
        ticket: ResidencyTicket,
        allocation: ArenaAllocation,
        cluster_allocation: Option<ArenaAllocation>,
        cluster_count: u32,
        kind: PayloadKind,
        target: Option<ChunkDomainRef>,
        span: ChunkSpan,
        byte_len: u64,
        stride: u32,
        attribute_kind: Option<AttributeKind>,
        relation_layout: Option<RelationUploadLayout>,
    },
}

impl TrackedGenericChunk {
    const fn ticket(self) -> ResidencyTicket {
        match self {
            Self::Uploading { ticket, .. } | Self::Resident { ticket, .. } => ticket,
        }
    }

    const fn allocation(self) -> ArenaAllocation {
        match self {
            Self::Uploading { allocation, .. } | Self::Resident { allocation, .. } => allocation,
        }
    }

    const fn cluster_allocation(self) -> Option<ArenaAllocation> {
        match self {
            Self::Uploading {
                cluster_allocation, ..
            }
            | Self::Resident {
                cluster_allocation, ..
            } => cluster_allocation,
        }
    }
}

impl<D: Device> ChunkGpuResidency<D> {
    pub(super) fn poll_generic(
        &mut self,
        completed: FenceValue,
    ) -> Result<(), ChunkResidencyError> {
        let mut index = 0;
        while index < self.generic.len() {
            let TrackedGenericChunk::Uploading {
                ticket,
                allocation,
                cluster_allocation,
                cluster_count,
                fence,
                kind,
                target,
                span,
                byte_len,
                stride,
                attribute_kind,
                relation_layout,
                cancelled,
            } = self.generic[index]
            else {
                index += 1;
                continue;
            };
            if fence > completed {
                index += 1;
                continue;
            }
            if cancelled {
                self.arena.release(allocation)?;
                self.release_generic_cluster(cluster_allocation)?;
                self.remove_generic(index);
            } else {
                self.generic[index] = TrackedGenericChunk::Resident {
                    ticket,
                    allocation,
                    cluster_allocation,
                    cluster_count,
                    kind,
                    target,
                    span,
                    byte_len,
                    stride,
                    attribute_kind,
                    relation_layout,
                };
                self.completed.push(ticket);
                self.bump_revision();
                index += 1;
            }
        }
        Ok(())
    }

    pub(in crate::engine) fn resident_generic(
        &self,
        ticket: ResidencyTicket,
    ) -> Option<ResidentGenericChunk> {
        let index = *self.generic_index.get(&ticket_key(ticket))?;
        match *self.generic.get(index)? {
            TrackedGenericChunk::Resident {
                ticket: owner,
                allocation,
                kind,
                target,
                span,
                byte_len,
                stride,
                relation_layout,
                ..
            } if owner == ticket => Some(ResidentGenericChunk {
                ticket,
                kind,
                target,
                byte_offset: allocation.byte_offset(),
                byte_len,
                local_rows: span.row_count(),
                stride: relation_layout.map_or(stride, RelationUploadLayout::uniform_stride),
            }),
            TrackedGenericChunk::Uploading { .. } | TrackedGenericChunk::Resident { .. } => None,
        }
    }

    pub(super) fn resident_point_range(
        &self,
        ticket: ResidencyTicket,
    ) -> Option<ResidentChunkRange> {
        let index = *self.generic_index.get(&ticket_key(ticket))?;
        match *self.generic.get(index)? {
            TrackedGenericChunk::Resident {
                ticket: owner,
                allocation,
                cluster_allocation: Some(cluster_allocation),
                cluster_count,
                kind: PayloadKind::PointBatch,
                span,
                ..
            } if owner == ticket => Some(ResidentChunkRange {
                ticket,
                coordinate_allocation: allocation,
                cluster_allocation,
                cluster_count,
                radius_base: 0,
                max_radius: 0.0,
                span,
            }),
            _ => None,
        }
    }

    pub(super) fn resident_instance_plan(
        &self,
        placement: &crate::engine::InstanceChunkPlacement,
    ) -> Option<ResidentInstanceChunkPlacement> {
        let ticket = placement.ticket();
        let index = *self.generic_index.get(&ticket_key(ticket))?;
        match *self.generic.get(index)? {
            TrackedGenericChunk::Resident {
                ticket: owner,
                allocation,
                kind: PayloadKind::InstanceBatch,
                span,
                ..
            } if owner == ticket => Some(ResidentInstanceChunkPlacement {
                id: placement.id(),
                ticket,
                byte_offset: allocation.byte_offset(),
                span,
                template: std::sync::Arc::clone(placement.template()),
                color: placement.color(),
                timeline: None,
            }),
            _ => None,
        }
    }

    pub(super) fn resident_instance_source(&self, ticket: ResidencyTicket) -> Option<(u64, u32)> {
        let index = *self.generic_index.get(&ticket_key(ticket))?;
        match *self.generic.get(index)? {
            TrackedGenericChunk::Resident {
                ticket: owner,
                allocation,
                kind: PayloadKind::InstanceBatch,
                span,
                ..
            } if owner == ticket => Some((allocation.byte_offset(), span.row_count())),
            _ => None,
        }
    }

    pub(super) fn resident_relation_plan(
        &self,
        placement: &crate::engine::RelationChunkPlacement,
    ) -> Option<crate::engine::chunk_draw_plan::ResidentRelationChunkPlacement> {
        let ticket = placement.ticket();
        let index = *self.generic_index.get(&ticket_key(ticket))?;
        match *self.generic.get(index)? {
            TrackedGenericChunk::Resident {
                ticket: owner,
                kind: PayloadKind::RelationBatch,
                span,
                ..
            } if owner == ticket => {
                let relations =
                    std::sync::Arc::clone(self.relation_sources.get(&ticket_key(ticket))?);
                Some(
                    crate::engine::chunk_draw_plan::ResidentRelationChunkPlacement {
                        id: placement.id(),
                        ticket,
                        span,
                        relations,
                        style: placement.style(),
                        visual: placement.visual().map(std::sync::Arc::clone),
                    },
                )
            }
            _ => None,
        }
    }

    pub(super) fn resident_attribute_column(
        &self,
        ticket: ResidencyTicket,
    ) -> Option<crate::engine::chunk_draw_plan::ResidentAttributeColumn> {
        let index = *self.generic_index.get(&ticket_key(ticket))?;
        match *self.generic.get(index)? {
            TrackedGenericChunk::Resident {
                ticket: owner,
                allocation,
                kind: PayloadKind::Attribute,
                target: Some(target),
                span,
                byte_len,
                attribute_kind: Some(kind),
                ..
            } if owner == ticket => Some(crate::engine::chunk_draw_plan::ResidentAttributeColumn {
                ticket,
                target,
                byte_offset: allocation.byte_offset(),
                byte_len,
                local_rows: span.row_count(),
                kind,
                timeline: None,
            }),
            _ => None,
        }
    }

    pub(super) fn resident_visual_attribute_column(
        &self,
        ticket: ResidencyTicket,
    ) -> Option<crate::engine::chunk_draw_plan::ResidentAttributeColumn> {
        let mut column = self.resident_attribute_column(ticket)?;
        let Ok(index) = self
            .attribute_windows
            .binary_search_by_key(&ticket, |window| window.attribute)
        else {
            return Some(column);
        };
        let window = self.attribute_windows[index];
        let start = self.resident_attribute_column(window.start)?;
        let end = self.resident_attribute_column(window.end)?;
        if start.target != column.target
            || end.target != column.target
            || start.kind != column.kind
            || end.kind != column.kind
            || start.local_rows != column.local_rows
            || end.local_rows != column.local_rows
        {
            return None;
        }
        column.timeline = Some(
            crate::engine::chunk_draw_plan::ResidentAttributeChunkWindow {
                start_byte_offset: start.byte_offset,
                end_byte_offset: end.byte_offset,
                interpolation: window.interpolation,
                materialized_byte_offset: self
                    .attribute_materialization(ticket)
                    .map(|entry| entry.output_byte_offset),
            },
        );
        Some(column)
    }

    pub(super) fn cancel_generic(&mut self, ticket: ResidencyTicket) {
        let Some(&index) = self.generic_index.get(&ticket_key(ticket)) else {
            return;
        };
        if let TrackedGenericChunk::Uploading { cancelled, .. } = &mut self.generic[index] {
            *cancelled = true;
        }
    }

    pub(super) fn evict_generic(
        &mut self,
        key: ResidencyKey,
        generation: u64,
    ) -> Result<(), ChunkResidencyError> {
        let Some(&index) = self.generic_index.get(&(key, generation)) else {
            return Ok(());
        };
        match self.generic[index] {
            TrackedGenericChunk::Uploading { .. } => {
                if let TrackedGenericChunk::Uploading { cancelled, .. } = &mut self.generic[index] {
                    *cancelled = true;
                }
            }
            TrackedGenericChunk::Resident { .. } => {
                let entry = self.generic[index];
                self.arena.release(entry.allocation())?;
                self.release_generic_cluster(entry.cluster_allocation())?;
                self.remove_generic(index);
                self.bump_revision();
            }
        }
        Ok(())
    }

    pub(super) fn discard_generic(
        &mut self,
        ticket: ResidencyTicket,
    ) -> Result<(), ChunkResidencyError> {
        let Some(&index) = self.generic_index.get(&ticket_key(ticket)) else {
            return Ok(());
        };
        self.arena.release(self.generic[index].allocation())?;
        self.release_generic_cluster(self.generic[index].cluster_allocation())?;
        self.remove_generic(index);
        self.bump_revision();
        Ok(())
    }

    fn insert_generic(&mut self, entry: &TrackedGenericChunk) {
        let index = self.generic.len();
        self.generic_index.insert(ticket_key(entry.ticket()), index);
        self.generic.push(*entry);
    }

    fn remove_generic(&mut self, index: usize) -> Option<TrackedGenericChunk> {
        let removed = self.generic.get(index).copied()?;
        self.generic_index.remove(&ticket_key(removed.ticket()));
        self.relation_sources.remove(&ticket_key(removed.ticket()));
        let removed = self.generic.swap_remove(index);
        if let Some(moved) = self.generic.get(index).copied() {
            self.generic_index.insert(ticket_key(moved.ticket()), index);
        }
        Some(removed)
    }

    fn point_clusters(
        &mut self,
        payload: &ChunkPayload,
    ) -> Result<(Option<ArenaAllocation>, u32), ChunkResidencyError> {
        let ChunkPayload::PointBatch(points) = payload else {
            self.cluster_scratch.clear();
            return Ok((None, 0));
        };
        self.build_clusters(points.positions().as_ref())?;
        let bytes = bytemuck::cast_slice::<ChunkClusterGpu, u8>(self.cluster_scratch.as_slice());
        let allocation = self
            .cluster_arena
            .allocate(u64::try_from(bytes.len()).map_err(|_| ChunkResidencyError::SizeOverflow)?)?;
        let count = u32::try_from(self.cluster_scratch.len())
            .map_err(|_| ChunkResidencyError::LocalAddressOverflow)?;
        Ok((Some(allocation), count))
    }

    fn release_generic_cluster(
        &mut self,
        allocation: Option<ArenaAllocation>,
    ) -> Result<(), ChunkResidencyError> {
        if let Some(allocation) = allocation {
            self.cluster_arena.release(allocation)?;
        }
        Ok(())
    }
}
