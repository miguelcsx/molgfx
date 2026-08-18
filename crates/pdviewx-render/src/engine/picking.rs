//! Constant-time picking from the exact integer gbuffer silhouette.

use super::Engine;
use crate::error::RenderError;
use crate::passes::{
    ENTITY_RESOURCE, SEGMENT_LABEL_RESOURCE, SEGMENT_VOLUME_RESOURCE, STRUCTURE_RESOURCE,
};
use pdviewx_core::{AtomSelection, EntityId, EntityKind, EntityRef, VolumeSegmentRef};
use pdviewx_gpu::{BufferDesc, BufferUsage, CommandEncoder as _, Device, Queue as _};

const READBACK_BYTES: u32 = 256;

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
    Structure(EntityRef),
    /// A caller-supplied categorical volume label.
    VolumeSegment(VolumeSegmentRef),
}

#[derive(Debug)]
pub(crate) struct Picker<D: Device> {
    entity: D::Buffer,
    structure: D::Buffer,
    segment_volume: D::Buffer,
    segment_label: D::Buffer,
}

impl<D: Device> Picker<D> {
    pub(crate) fn new(device: &D) -> Result<Self, RenderError> {
        Ok(Self {
            entity: readback(device, "entity pick readback")?,
            structure: readback(device, "structure pick readback")?,
            segment_volume: readback(device, "segment volume pick readback")?,
            segment_label: readback(device, "segment label pick readback")?,
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
        if !self.record_pick(x, y) {
            return Ok(None);
        }
        let entity = self
            .queue
            .read_buffer_async(
                &self.device,
                &self.picker.entity,
                0,
                std::mem::size_of::<u32>() as u64,
            )
            .await?;
        let structure = self
            .queue
            .read_buffer_async(
                &self.device,
                &self.picker.structure,
                0,
                std::mem::size_of::<u32>() as u64,
            )
            .await?;
        let segment_volume = self
            .queue
            .read_buffer_async(
                &self.device,
                &self.picker.segment_volume,
                0,
                std::mem::size_of::<u32>() as u64,
            )
            .await?;
        let segment_label = self
            .queue
            .read_buffer_async(
                &self.device,
                &self.picker.segment_label,
                0,
                std::mem::size_of::<u32>() as u64,
            )
            .await?;
        Ok(self.resolve_pick(&entity, &structure, &segment_volume, &segment_label))
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
        if !self.record_pick(x, y) {
            return Ok(None);
        }
        let entity = self.queue.read_buffer_blocking(
            &self.device,
            &self.picker.entity,
            0,
            std::mem::size_of::<u32>() as u64,
        )?;
        let structure = self.queue.read_buffer_blocking(
            &self.device,
            &self.picker.structure,
            0,
            std::mem::size_of::<u32>() as u64,
        )?;
        let segment_volume = self.queue.read_buffer_blocking(
            &self.device,
            &self.picker.segment_volume,
            0,
            std::mem::size_of::<u32>() as u64,
        )?;
        let segment_label = self.queue.read_buffer_blocking(
            &self.device,
            &self.picker.segment_label,
            0,
            std::mem::size_of::<u32>() as u64,
        )?;
        Ok(self.resolve_pick(&entity, &structure, &segment_volume, &segment_label))
    }

    fn record_pick(&mut self, x: u32, y: u32) -> bool {
        if x >= self.width || y >= self.height {
            return false;
        }
        let Some(pool) = &self.pool else {
            return false;
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
            return false;
        };
        let mut encoder = self.device.create_command_encoder();
        encoder.copy_texture_to_buffer(
            entity_texture,
            (x, y),
            (1, 1),
            READBACK_BYTES,
            &self.picker.entity,
        );
        encoder.copy_texture_to_buffer(
            structure_texture,
            (x, y),
            (1, 1),
            READBACK_BYTES,
            &self.picker.structure,
        );
        encoder.copy_texture_to_buffer(
            segment_volume_texture,
            (x, y),
            (1, 1),
            READBACK_BYTES,
            &self.picker.segment_volume,
        );
        encoder.copy_texture_to_buffer(
            segment_label_texture,
            (x, y),
            (1, 1),
            READBACK_BYTES,
            &self.picker.segment_label,
        );
        self.queue.submit(encoder);
        true
    }

    fn resolve_pick(
        &self,
        entity: &[u8],
        structure: &[u8],
        segment_volume: &[u8],
        segment_label: &[u8],
    ) -> Option<Pick> {
        if let (Some(source_id), Some(label)) = (read_u32(segment_volume), read_u32(segment_label))
            && source_id != u32::MAX
            && let Some(segment) = self.scene_gpu.resolve_segment(source_id, label)
        {
            return Some(Pick {
                entity: PickEntity::VolumeSegment(segment),
                selection: AtomSelection::Empty,
            });
        }
        let (Some(entity), Some(structure)) = (read_u32(entity), read_u32(structure)) else {
            return None;
        };
        let entity = self.scene_gpu.resolve_entity(structure, EntityId(entity))?;
        let selection = match entity.kind {
            EntityKind::Atom => AtomSelection::Sparse(vec![entity.index]),
            _ => AtomSelection::Empty,
        };
        Some(Pick {
            entity: PickEntity::Structure(entity),
            selection,
        })
    }
}

fn readback<D: Device>(device: &D, label: &'static str) -> Result<D::Buffer, RenderError> {
    Ok(device.create_buffer(&BufferDesc {
        label,
        size: u64::from(READBACK_BYTES),
        usage: BufferUsage::COPY_DST.union(BufferUsage::MAP_READ),
    })?)
}

fn read_u32(bytes: &[u8]) -> Option<u32> {
    let slice = bytes.get(..std::mem::size_of::<u32>())?;
    let mut array = [0; std::mem::size_of::<u32>()];
    array.copy_from_slice(slice);
    Some(u32::from_le_bytes(array))
}
