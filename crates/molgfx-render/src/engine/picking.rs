//! Constant-time picking from the exact integer gbuffer silhouette.

use super::Engine;
use crate::error::RenderError;
use crate::passes::{
    ENTITY_RESOURCE, SEGMENT_LABEL_RESOURCE, SEGMENT_VOLUME_RESOURCE, STRUCTURE_RESOURCE,
};
use molgfx_core::{
    AtomSelection, EntityKind, GlobalPickIdentity, GpuPickToken, PickPageTicket, VolumeSegmentRef,
};
use molgfx_gpu::{BufferDesc, BufferUsage, CommandEncoder as _, Device, Queue as _};

const READBACK_BYTES: u32 = 256;
const PICK_FIELDS: u64 = 4;

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
pub(crate) struct Picker<D: Device> {
    readback: D::Buffer,
    submission: Box<[Option<PickPageTicket>]>,
}

impl<D: Device> Picker<D> {
    pub(crate) fn new(device: &D, page_capacity: u32) -> Result<Self, RenderError> {
        let capacity = usize::try_from(page_capacity)
            .map_err(|_| molgfx_core::PickingError::CapacityTooLarge)?;
        let mut submission = Vec::new();
        submission
            .try_reserve_exact(capacity)
            .map_err(|_| molgfx_core::PickingError::AllocationFailed)?;
        submission.resize(capacity, None);
        Ok(Self {
            readback: readback(device, "packed pick readback")?,
            submission: submission.into_boxed_slice(),
        })
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
        if !self.record_pick(x, y)? {
            return Ok(None);
        }
        let packed = self
            .queue
            .read_buffer_async(
                &self.device,
                &self.picker.readback,
                0,
                u64::from(READBACK_BYTES) * PICK_FIELDS,
            )
            .await?;
        self.resolve_pick(&packed)
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
        if !self.record_pick(x, y)? {
            return Ok(None);
        }
        let packed = self.queue.read_buffer_blocking(
            &self.device,
            &self.picker.readback,
            0,
            u64::from(READBACK_BYTES) * PICK_FIELDS,
        )?;
        self.resolve_pick(&packed)
    }

    fn record_pick(&mut self, x: u32, y: u32) -> Result<bool, RenderError> {
        if x >= self.width || y >= self.height {
            return Ok(false);
        }
        let Some(pool) = &self.pool else {
            return Ok(false);
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
            return Ok(false);
        };
        self.scene_gpu
            .capture_pick_submission(&mut self.picker.submission)?;
        let mut encoder = self.device.create_command_encoder();
        encoder.copy_texture_to_buffer(
            entity_texture,
            (x, y),
            (1, 1),
            READBACK_BYTES,
            0,
            &self.picker.readback,
        );
        encoder.copy_texture_to_buffer(
            structure_texture,
            (x, y),
            (1, 1),
            READBACK_BYTES,
            u64::from(READBACK_BYTES),
            &self.picker.readback,
        );
        encoder.copy_texture_to_buffer(
            segment_volume_texture,
            (x, y),
            (1, 1),
            READBACK_BYTES,
            u64::from(READBACK_BYTES) * 2,
            &self.picker.readback,
        );
        encoder.copy_texture_to_buffer(
            segment_label_texture,
            (x, y),
            (1, 1),
            READBACK_BYTES,
            u64::from(READBACK_BYTES) * 3,
            &self.picker.readback,
        );
        self.queue.submit(encoder);
        Ok(true)
    }

    fn resolve_pick(&self, packed: &[u8]) -> Result<Option<Pick>, RenderError> {
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
        let identity = self
            .scene_gpu
            .resolve_global_pick(token, &self.picker.submission)?;
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
    pub(crate) fn capture_pick_submission_for_test(&mut self) -> Result<(), RenderError> {
        self.scene_gpu
            .capture_pick_submission(&mut self.picker.submission)
    }

    #[cfg(test)]
    pub(crate) fn resolve_pick_token_for_test(
        &self,
        token: GpuPickToken,
    ) -> Result<GlobalPickIdentity, RenderError> {
        self.scene_gpu
            .resolve_global_pick(token, &self.picker.submission)
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
