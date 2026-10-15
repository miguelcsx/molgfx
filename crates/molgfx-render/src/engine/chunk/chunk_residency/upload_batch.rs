//! Batched, single-copy device uploads for every paged provider payload.
//!
//! Staging only fills the host ring. The device side runs once per epoch: the
//! staged byte ranges are copied host-side into one reusable staging buffer,
//! recorded as `copy_buffer_to_buffer` work in one encoder, and submitted
//! under a single fence. Submissions are therefore bounded by frames rather
//! than by chunks, no payload is written into the arena twice, and the ring is
//! the only host copy of a staged payload.

use super::ChunkGpuResidency;
use super::generic::TrackedGenericChunk;
use super::tracked::TrackedChunk;
use super::trajectory::TrackedFrame;
use crate::engine::ChunkResidencyError;
use crate::residency::LazyUploadRing;
use molgfx_core::ResidencyTicket;
use molgfx_gpu::{
    BufferDesc, BufferUsage, CommandEncoder as _, Device, FenceValue, Queue as _,
    UploadReservation, UploadState,
};

const STAGING_LABEL: &str = "resident upload staging";
const MINIMUM_STAGING_BYTES: u64 = 256;

/// Device arena receiving one staged byte range.
#[derive(Clone, Copy, Debug)]
pub(in crate::engine) enum CopyTarget {
    /// Paged coordinate arena shared by every paged consumer.
    Canonical,
    /// Bounded trajectory-frame arena.
    Frame,
    /// Bounded cluster arena.
    Cluster,
}

/// One staged byte range and the device rows it becomes.
#[derive(Clone, Copy, Debug)]
pub(in crate::engine) struct StagedCopy {
    pub(super) reservation: UploadReservation,
    /// The residency ticket this staged range belongs to.
    pub(super) entry: ResidencyTicket,
    pub(super) source_offset: usize,
    pub(super) byte_len: usize,
    pub(super) target: CopyTarget,
    pub(super) target_offset: u64,
}

/// Reserves one epoch range holding every segment of a single chunk.
///
/// The reservation is one ticket, so a chunk's segments are submitted and
/// retired together. Nothing reaches the device until the epoch flushes.
///
/// # Errors
///
/// Returns typed address-overflow, staging or admission failures.
pub(in crate::engine) fn stage_segments(
    uploads: &mut LazyUploadRing,
    staged: &mut Vec<StagedCopy>,
    ticket: ResidencyTicket,
    segments: &[(&[u8], CopyTarget, u64)],
) -> Result<(), ChunkResidencyError> {
    let mut total = 0_usize;
    for (bytes, _, _) in segments {
        total = total
            .checked_add(bytes.len())
            .ok_or(ChunkResidencyError::SizeOverflow)?;
    }
    if total == 0 {
        return Err(ChunkResidencyError::EmptyChunk);
    }
    let reservation = uploads.ensure()?.reserve(total)?;
    let mut cursor = reservation.offset();
    {
        let ring = uploads.ensure()?;
        let staging = ring.bytes_mut(reservation)?;
        for (bytes, _, _) in segments {
            let end = cursor
                .checked_add(bytes.len())
                .ok_or(ChunkResidencyError::SizeOverflow)?;
            let Some(destination) = staging
                .get_mut(cursor - reservation.offset()..)
                .and_then(|tail| tail.get_mut(..bytes.len()))
            else {
                return Err(ChunkResidencyError::SizeOverflow);
            };
            destination.copy_from_slice(bytes);
            cursor = end;
        }
        ring.commit(reservation.ticket())?;
    }
    if let Err(error) = uploads.ensure()?.ensure_submittable(reservation.ticket()) {
        uploads.ensure()?.cancel(reservation.ticket())?;
        return Err(error.into());
    }
    let mut offset = reservation.offset();
    for (bytes, target, target_offset) in segments {
        if bytes.is_empty() {
            continue;
        }
        staged.push(StagedCopy {
            reservation,
            entry: ticket,
            source_offset: offset,
            byte_len: bytes.len(),
            target: *target,
            target_offset: *target_offset,
        });
        offset += bytes.len();
    }
    Ok(())
}

impl<D: Device> ChunkGpuResidency<D> {
    /// Writes the epoch's staged bytes once and submits every arena copy.
    ///
    /// Chunks staged since the previous flush travel together: one staging
    /// write, one encoder and one tracked submission.
    ///
    /// # Errors
    ///
    /// Returns typed host address, staging, allocation, admission or device
    /// failures.
    pub(in crate::engine) fn flush(
        &mut self,
        device: &D,
        queue: &D::Queue,
    ) -> Result<(), ChunkResidencyError> {
        if self.staged.is_empty() {
            return Ok(());
        }
        let staged = std::mem::take(&mut self.staged);
        let mut span = 0_usize;
        for copy in &staged {
            span = span.max(
                copy.source_offset
                    .checked_add(copy.byte_len)
                    .ok_or(ChunkResidencyError::SizeOverflow)?,
            );
        }
        let arena_bytes = usize::try_from(self.arena.capacity_bytes())
            .map_err(|_| ChunkResidencyError::SizeOverflow)?;
        let required = span.max(arena_bytes.min(self.config.uploads.capacity_bytes));
        self.ensure_staging_buffer(device, required)?;
        let mut staging = std::mem::take(&mut self.staging);
        if staging.len() < required {
            staging.resize(required, 0);
        }
        staging[..span].copy_from_slice(&self.uploads.ensure()?.staging_bytes()[..span]);
        let Some(buffer) = self.staging_buffer.as_ref() else {
            self.staging = staging;
            return Err(ChunkResidencyError::SizeOverflow);
        };
        queue.write_buffer(buffer, 0, &staging[..span]);
        let mut encoder = device.create_command_encoder();
        for copy in &staged {
            let destination = match copy.target {
                CopyTarget::Canonical => &self.buffer,
                CopyTarget::Frame => &self.frame_buffer,
                CopyTarget::Cluster => &self.cluster_buffer,
            };
            encoder.copy_buffer_to_buffer(
                buffer,
                u64::try_from(copy.source_offset).map_err(|_| ChunkResidencyError::SizeOverflow)?,
                destination,
                copy.target_offset,
                u64::try_from(copy.byte_len).map_err(|_| ChunkResidencyError::SizeOverflow)?,
            );
        }
        let fence = queue.submit_tracked(encoder);
        let mut submitted = false;
        for copy in &staged {
            if self.uploads.ensure()?.state(copy.reservation.ticket()) == Some(UploadState::Ready) {
                self.uploads
                    .ensure()?
                    .submit(copy.reservation.ticket(), fence)?;
                submitted = true;
            }
        }
        self.staging = staging;
        if submitted {
            self.attach_fence(&staged, fence);
        }
        Ok(())
    }

    fn ensure_staging_buffer(
        &mut self,
        device: &D,
        bytes: usize,
    ) -> Result<(), ChunkResidencyError> {
        let size = u64::try_from(bytes)
            .map_err(|_| ChunkResidencyError::SizeOverflow)?
            .max(MINIMUM_STAGING_BYTES);
        if self.staging_bytes >= size {
            return Ok(());
        }
        self.staging_buffer = Some(
            device.create_buffer(&BufferDesc {
                label: STAGING_LABEL,
                size,
                usage: BufferUsage::COPY_SRC
                    .union(BufferUsage::COPY_DST)
                    .union(BufferUsage::STORAGE),
            })?,
        );
        self.staging_bytes = size;
        Ok(())
    }

    /// Publishes the submitting fence to every chunk that submission carries.
    ///
    /// An upload is only promotable once its bytes have a fence, so a chunk
    /// staged but not yet flushed stays unmaterializable and never becomes
    /// resident.
    fn attach_fence(&mut self, staged: &[StagedCopy], fence_value: FenceValue) {
        // A chunk is promoted by the fence of the epoch that carried it, so
        // match each tracked entry against the tickets staged for this flush.
        let submitted = |entry: ResidencyTicket| staged.iter().any(|copy| copy.entry == entry);
        for entry in &mut self.tracked {
            if let TrackedChunk::Uploading {
                ticket,
                fence: slot @ None,
                ..
            } = entry
                && submitted(*ticket)
            {
                *slot = Some(fence_value);
            }
        }
        for entry in &mut self.frames {
            if let TrackedFrame::Uploading { ticket, fence, .. } = entry
                && submitted(*ticket)
            {
                *fence = fence_of(*fence).max(fence_value);
            }
        }
        for entry in &mut self.generic {
            if let TrackedGenericChunk::Uploading { ticket, fence, .. } = entry
                && submitted(*ticket)
            {
                *fence = fence_of(*fence).max(fence_value);
            }
        }
    }
}

/// The fence a tracked entry already carries, as a comparable value.
///
/// A pending upload carries `FenceValue::default()`, which sorts below every
/// issued fence, so taking the maximum promotes it exactly once.
fn fence_of(value: FenceValue) -> FenceValue {
    value
}
