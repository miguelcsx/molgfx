//! Constant-time picking from the exact integer gbuffer silhouette.

use super::Engine;
use crate::error::RenderError;
use crate::passes::{
    ENTITY_RESOURCE, SEGMENT_LABEL_RESOURCE, SEGMENT_VOLUME_RESOURCE, STRUCTURE_RESOURCE,
};
use molgfx_core::{
    AtomSelection, EntityKind, GlobalPickIdentity, GpuPickToken, PickPageTicket, VolumeSegmentRef,
};
use molgfx_gpu::{BufferDesc, BufferUsage, CommandEncoder as _, Device, Queue as _, Readback as _};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

const READBACK_BYTES: u32 = 256;
const PICK_FIELDS: u64 = 4;

/// Picks that may be awaiting readback at once; further requests are refused
/// rather than queued, so backpressure is explicit and memory is bounded.
const PICKS_IN_FLIGHT: usize = 4;

/// Bytes a pick readback resolves: one field per identity domain.
pub const PICK_READBACK_BYTES: u64 = READBACK_BYTES as u64 * PICK_FIELDS;

/// A resolved visible entity and its convenient single-atom selection.
#[derive(Clone, Debug)]
pub struct Pick {
    /// Exact scene entity written by the visible fragment.
    pub entity: PickEntity,
    /// A one-atom selection for atom-backed entities; empty otherwise.
    pub selection: AtomSelection,
}

/// The two identity domains that can be visible in a frame.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PickEntity {
    /// A molecular entity from the opaque or transparent structural path.
    Structure(GlobalPickIdentity),
    /// A caller-supplied categorical volume label.
    VolumeSegment(VolumeSegmentRef),
}

#[derive(Debug)]
struct PickBuffer<D: Device> {
    buffer: D::Buffer,
    busy: Arc<AtomicBool>,
}

/// Bounded pool of readback buffers, one per pick in flight.
#[derive(Debug)]
pub(crate) struct Picker<D: Device> {
    buffers: Box<[PickBuffer<D>]>,
    page_capacity: usize,
}

impl<D: Device> Picker<D> {
    pub(crate) fn new(device: &D, page_capacity: u32) -> Result<Self, RenderError> {
        let page_capacity = usize::try_from(page_capacity)
            .map_err(|_| molgfx_core::PickingError::CapacityTooLarge)?;
        let mut buffers = Vec::new();
        buffers
            .try_reserve_exact(PICKS_IN_FLIGHT)
            .map_err(|_| molgfx_core::PickingError::AllocationFailed)?;
        for _ in 0..PICKS_IN_FLIGHT {
            buffers.push(PickBuffer {
                buffer: readback(device, "packed pick readback")?,
                busy: Arc::new(AtomicBool::new(false)),
            });
        }
        Ok(Self {
            buffers: buffers.into_boxed_slice(),
            page_capacity,
        })
    }

    /// Claims a free readback buffer, or reports that every one is awaited.
    fn acquire(&self) -> Result<(usize, Arc<AtomicBool>), RenderError> {
        for (index, slot) in self.buffers.iter().enumerate() {
            if slot
                .busy
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return Ok((index, Arc::clone(&slot.busy)));
            }
        }
        Err(molgfx_core::PickingError::InFlightExhausted.into())
    }
}

/// One recorded pick: its own readback buffer and the page generations that
/// were resident when it was submitted.
///
/// Both belong to the request, so another pick, a later frame or a recycled
/// page cannot change which entity these bytes name. Dropping the value frees
/// its readback buffer for the next request.
pub struct PendingPick<D: Device> {
    readback: D::Readback,
    submission: Box<[Option<PickPageTicket>]>,
    busy: Arc<AtomicBool>,
}

impl<D: Device> std::fmt::Debug for PendingPick<D> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PendingPick")
            .field("pages", &self.submission.len())
            .finish_non_exhaustive()
    }
}

impl<D: Device> PendingPick<D> {
    /// Awaits the packed identity bytes without borrowing the engine.
    ///
    /// # Errors
    ///
    /// Returns a typed device error when the mapped readback fails.
    pub async fn resolve(&self) -> Result<Vec<u8>, RenderError> {
        Ok(self
            .readback
            .resolve(0, u64::from(READBACK_BYTES) * PICK_FIELDS)
            .await?)
    }
}

impl<D: Device> Drop for PendingPick<D> {
    fn drop(&mut self) {
        self.busy.store(false, Ordering::Release);
    }
}

impl<D: Device> Engine<D> {
    /// Asynchronously resolves the exact visible entity at one
    /// top-left-origin pixel. This is the portable browser entry point.
    ///
    /// # Errors
    ///
    /// Returns a typed device error when readback fails. A pixel outside the
    /// target or over the background resolves to `Ok(None)`.
    pub async fn pick_async(&mut self, x: u32, y: u32) -> Result<Option<Pick>, RenderError> {
        let Some(pending) = self.begin_pick(x, y)? else {
            return Ok(None);
        };
        let packed = pending.resolve().await?;
        self.finish_pick(&pending, &packed)
    }

    /// Records one pick and returns a detached readback handle.
    ///
    /// The handle borrows neither the engine nor its device, so on the
    /// browser's single JavaScript thread a frame may render, and further
    /// picks may begin, between this call and [`Self::finish_pick`]. `None`
    /// means the pixel is outside the target or nothing is drawable.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the pick copies cannot be recorded, or when
    /// every readback buffer is already awaited.
    pub fn begin_pick(&mut self, x: u32, y: u32) -> Result<Option<PendingPick<D>>, RenderError> {
        let (index, busy) = self.picker.acquire()?;
        let submission = match self.record_pick(index, x, y) {
            Ok(Some(submission)) => submission,
            Ok(None) => {
                busy.store(false, Ordering::Release);
                return Ok(None);
            }
            Err(error) => {
                busy.store(false, Ordering::Release);
                return Err(error);
            }
        };
        let readback = self
            .queue
            .readback(&self.device, &self.picker.buffers[index].buffer);
        Ok(Some(PendingPick {
            readback,
            submission,
            busy,
        }))
    }

    /// Resolves the packed bytes a [`Self::begin_pick`] produced, against the
    /// page generations that request captured.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the packed identity cannot be resolved.
    pub fn finish_pick(
        &self,
        pending: &PendingPick<D>,
        packed: &[u8],
    ) -> Result<Option<Pick>, RenderError> {
        self.resolve_pick(packed, &pending.submission)
    }

    /// Resolves the exact visible entity at one top-left-origin pixel in
    /// constant time with respect to scene size.
    ///
    /// # Errors
    ///
    /// Returns a typed device error when readback fails. A pixel outside the
    /// target or over the background resolves to `Ok(None)`.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn pick(&mut self, x: u32, y: u32) -> Result<Option<Pick>, RenderError> {
        let Some(pending) = self.begin_pick(x, y)? else {
            return Ok(None);
        };
        let index = self
            .picker
            .buffers
            .iter()
            .position(|slot| Arc::ptr_eq(&slot.busy, &pending.busy))
            .ok_or(molgfx_core::PickingError::InFlightExhausted)?;
        let packed = self.queue.read_buffer_blocking(
            &self.device,
            &self.picker.buffers[index].buffer,
            0,
            PICK_READBACK_BYTES,
        )?;
        self.finish_pick(&pending, &packed)
    }

    /// Copies the four identity texels into buffer `index` and captures the
    /// page generations they were drawn against.
    fn record_pick(
        &mut self,
        index: usize,
        x: u32,
        y: u32,
    ) -> Result<Option<Box<[Option<PickPageTicket>]>>, RenderError> {
        if x >= self.width || y >= self.height {
            return Ok(None);
        }
        let Some(pool) = &self.pool else {
            return Ok(None);
        };
        let (
            Some(entity_texture),
            Some(structure_texture),
            Some(segment_volume_texture),
            Some(segment_label_texture),
        ) = (
            pool.texture(ENTITY_RESOURCE),
            pool.texture(STRUCTURE_RESOURCE),
            pool.texture(SEGMENT_VOLUME_RESOURCE),
            pool.texture(SEGMENT_LABEL_RESOURCE),
        )
        else {
            return Ok(None);
        };
        let mut submission = Vec::new();
        submission
            .try_reserve_exact(self.picker.page_capacity)
            .map_err(|_| molgfx_core::PickingError::AllocationFailed)?;
        submission.resize(self.picker.page_capacity, None);
        let mut submission = submission.into_boxed_slice();
        self.scene_gpu.capture_pick_submission(&mut submission)?;
        let destination = &self.picker.buffers[index].buffer;
        let mut encoder = self.device.create_command_encoder();
        encoder.copy_texture_to_buffer(
            entity_texture,
            (x, y),
            (1, 1),
            READBACK_BYTES,
            0,
            destination,
        );
        encoder.copy_texture_to_buffer(
            structure_texture,
            (x, y),
            (1, 1),
            READBACK_BYTES,
            u64::from(READBACK_BYTES),
            destination,
        );
        encoder.copy_texture_to_buffer(
            segment_volume_texture,
            (x, y),
            (1, 1),
            READBACK_BYTES,
            u64::from(READBACK_BYTES) * 2,
            destination,
        );
        encoder.copy_texture_to_buffer(
            segment_label_texture,
            (x, y),
            (1, 1),
            READBACK_BYTES,
            u64::from(READBACK_BYTES) * 3,
            destination,
        );
        self.queue.submit(encoder);
        Ok(Some(submission))
    }

    fn resolve_pick(
        &self,
        packed: &[u8],
        submission: &[Option<PickPageTicket>],
    ) -> Result<Option<Pick>, RenderError> {
        let local_row = pick_field(packed, 0);
        let resident_page = pick_field(packed, 1);
        let segment_volume = pick_field(packed, 2);
        let segment_label = pick_field(packed, 3);
        if let (Some(source_id), Some(label)) = (read_u32(segment_volume), read_u32(segment_label))
            && source_id != u32::MAX
            && let Some(segment) = self.scene_gpu.resolve_segment(source_id, label)
        {
            return Ok(Some(Pick {
                entity: PickEntity::VolumeSegment(segment),
                selection: AtomSelection::Empty,
            }));
        }
        let (Some(local_row), Some(resident_page)) = (read_u32(local_row), read_u32(resident_page))
        else {
            return Ok(None);
        };
        let token = GpuPickToken::new(resident_page, local_row);
        if token == GpuPickToken::NONE {
            return Ok(None);
        }
        let identity = self.scene_gpu.resolve_global_pick(token, submission)?;
        let selection = match (identity.kind(), u32::try_from(identity.row().get())) {
            (EntityKind::Atom, Ok(row)) => match row.checked_add(1) {
                Some(end) => AtomSelection::Range(row..end),
                None => AtomSelection::Empty,
            },
            _ => AtomSelection::Empty,
        };
        Ok(Some(Pick {
            entity: PickEntity::Structure(identity),
            selection,
        }))
    }

    #[cfg(test)]
    pub(crate) fn capture_pick_submission_for_test(
        &mut self,
    ) -> Result<Box<[Option<PickPageTicket>]>, RenderError> {
        let mut submission = vec![None; self.picker.page_capacity].into_boxed_slice();
        self.scene_gpu.capture_pick_submission(&mut submission)?;
        Ok(submission)
    }

    #[cfg(test)]
    pub(crate) fn resolve_pick_token_for_test(
        &self,
        token: GpuPickToken,
    ) -> Result<GlobalPickIdentity, RenderError> {
        let submission = vec![None; self.picker.page_capacity].into_boxed_slice();
        let mut submission = submission;
        self.scene_gpu.capture_pick_submission(&mut submission)?;
        self.scene_gpu.resolve_global_pick(token, &submission)
    }

    #[cfg(test)]
    pub(crate) fn resolve_pick_token_against_for_test(
        &self,
        token: GpuPickToken,
        submission: &[Option<PickPageTicket>],
    ) -> Result<GlobalPickIdentity, RenderError> {
        self.scene_gpu.resolve_global_pick(token, submission)
    }
}

fn readback<D: Device>(device: &D, label: &'static str) -> Result<D::Buffer, RenderError> {
    Ok(device.create_buffer(&BufferDesc {
        label,
        size: u64::from(READBACK_BYTES) * PICK_FIELDS,
        usage: BufferUsage::COPY_DST.union(BufferUsage::MAP_READ),
    })?)
}

fn pick_field(bytes: &[u8], index: usize) -> &[u8] {
    let width = READBACK_BYTES as usize;
    let start = index.saturating_mul(width);
    let end = start.saturating_add(width).min(bytes.len());
    match bytes.get(start..end) {
        Some(field) => field,
        None => &[],
    }
}

fn read_u32(bytes: &[u8]) -> Option<u32> {
    let slice = bytes.get(..std::mem::size_of::<u32>())?;
    let mut array = [0; std::mem::size_of::<u32>()];
    array.copy_from_slice(slice);
    Some(u32::from_le_bytes(array))
}
