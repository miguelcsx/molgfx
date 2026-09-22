//! Bounded provider-frame residency for topology-stable paged trajectories.
//!
//! Frame storage is proportional to the active working set. Upload and lookup
//! are `O(rows)` and amortized `O(1)` respectively; no operation traverses the
//! full trajectory.

use super::{ChunkGpuResidency, ticket_key};
use crate::engine::{ChunkResidencyError, ResidentTrajectoryChunk};
use molgfx_core::{ChunkData, ChunkPayload, ChunkSpan, LogicalRow, ResidencyKey, ResidencyTicket};
use molgfx_gpu::{ArenaAllocation, Device, FenceValue};

#[derive(Clone, Copy, Debug)]
pub(super) enum TrackedFrame {
    Uploading {
        ticket: ResidencyTicket,
        allocation: ArenaAllocation,
        span: ChunkSpan,
        fence: FenceValue,
        byte_len: u64,
        cancelled: bool,
    },
    Resident {
        ticket: ResidencyTicket,
        allocation: ArenaAllocation,
        span: ChunkSpan,
        byte_len: u64,
    },
}

impl TrackedFrame {
    pub(super) const fn ticket(self) -> ResidencyTicket {
        match self {
            Self::Uploading { ticket, .. } | Self::Resident { ticket, .. } => ticket,
        }
    }
}

impl<D: Device> ChunkGpuResidency<D> {
    pub(super) fn stage_frame(
        &mut self,
        device: &D,
        queue: &D::Queue,
        ticket: ResidencyTicket,
        data: &ChunkData,
    ) -> Result<(), ChunkResidencyError> {
        if self.frames.len() == self.frames.capacity() {
            return Err(ChunkResidencyError::TrackingCapacity);
        }
        let ChunkPayload::ProviderFrame(frame) = data.payload() else {
            return Err(ChunkResidencyError::UnsupportedPayload);
        };
        if frame.positions().is_empty() {
            return Err(ChunkResidencyError::EmptyChunk);
        }
        // A trajectory frame is what makes the interpolation target necessary:
        // the paged draw reads the second backing only while a window exists.
        self.ensure_frame_buffer(device)?;
        self.ensure_display_buffer(device)?;
        let bytes = bytemuck::cast_slice(frame.positions());
        let byte_len = u64::try_from(bytes.len()).map_err(|_| ChunkResidencyError::SizeOverflow)?;
        let allocation = self.frame_arena.allocate(byte_len)?;
        // A frame stages like every other payload: the bytes land in the host
        // ring here and reach the device in the frame's one epoch flush, so a
        // burst of frames costs one submission rather than one each.
        let segments = [(
            bytes,
            crate::engine::chunk_residency::CopyTarget::Frame,
            allocation.byte_offset(),
        )];
        if let Err(error) = crate::engine::chunk_residency::stage_segments(
            &mut self.uploads,
            &mut self.staged,
            ticket,
            &segments,
        ) {
            self.frame_arena.release(allocation)?;
            return Err(error);
        }
        let _ = queue;
        self.insert_frame(TrackedFrame::Uploading {
            ticket,
            allocation,
            span: ChunkSpan::new(
                LogicalRow::new(frame.descriptor().logical_start().get()),
                frame.descriptor().rows(),
            )?,
            fence: FenceValue::default(),
            byte_len,
            cancelled: false,
        });
        Ok(())
    }

    pub(super) fn poll_frames(
        &mut self,
        completed_fence: FenceValue,
    ) -> Result<(), ChunkResidencyError> {
        let mut index = 0;
        while index < self.frames.len() {
            let TrackedFrame::Uploading {
                ticket,
                allocation,
                span,
                fence,
                byte_len,
                cancelled,
            } = self.frames[index]
            else {
                index += 1;
                continue;
            };
            if fence > completed_fence {
                index += 1;
                continue;
            }
            if cancelled {
                self.frame_arena.release(allocation)?;
                self.remove_frame(index);
            } else {
                self.frames[index] = TrackedFrame::Resident {
                    ticket,
                    allocation,
                    span,
                    byte_len,
                };
                self.completed.push(ticket);
                self.bump_timeline_revision();
                index += 1;
            }
        }
        Ok(())
    }

    pub(in crate::engine) fn resident_frame(
        &self,
        ticket: ResidencyTicket,
    ) -> Option<ResidentTrajectoryChunk> {
        let index = *self.frame_index.get(&ticket_key(ticket))?;
        self.frames.get(index).and_then(|frame| match *frame {
            TrackedFrame::Resident {
                ticket: owner,
                allocation,
                span,
                byte_len,
            } if owner == ticket => Some(ResidentTrajectoryChunk {
                ticket,
                byte_offset: allocation.byte_offset(),
                byte_len,
                local_rows: span.row_count(),
            }),
            TrackedFrame::Uploading { .. } | TrackedFrame::Resident { .. } => None,
        })
    }

    pub(super) fn cancel_frame(
        &mut self,
        ticket: ResidencyTicket,
    ) -> Result<(), ChunkResidencyError> {
        let Some(&index) = self.frame_index.get(&ticket_key(ticket)) else {
            return Ok(());
        };
        match &mut self.frames[index] {
            TrackedFrame::Uploading { cancelled, .. } => *cancelled = true,
            TrackedFrame::Resident { allocation, .. } => {
                self.frame_arena.release(*allocation)?;
                self.remove_frame(index);
                self.bump_timeline_revision();
            }
        }
        Ok(())
    }

    pub(super) fn evict_frame(
        &mut self,
        key: ResidencyKey,
        generation: u64,
    ) -> Result<(), ChunkResidencyError> {
        let Some(&index) = self.frame_index.get(&(key, generation)) else {
            return Ok(());
        };
        let ticket = self.frames[index].ticket();
        self.cancel_frame(ticket)
    }

    fn insert_frame(&mut self, frame: TrackedFrame) {
        let index = self.frames.len();
        self.frame_index.insert(ticket_key(frame.ticket()), index);
        self.frames.push(frame);
    }

    fn remove_frame(&mut self, index: usize) -> Option<TrackedFrame> {
        let removed = self.frames.get(index).copied()?;
        self.frame_index.remove(&ticket_key(removed.ticket()));
        let removed = self.frames.swap_remove(index);
        if let Some(moved) = self.frames.get(index).copied() {
            self.frame_index.insert(ticket_key(moved.ticket()), index);
        }
        Some(removed)
    }
}
