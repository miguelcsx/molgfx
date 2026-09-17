//! Deterministic lowering of mixed relation anchors into homogeneous streams.

#[cfg(test)]
#[path = "relations_tests.rs"]
mod tests;

use crate::engine::ChunkResidencyError;
use molgfx_core::{ChunkEntityRef, PagedRelation, PagedSpatialAnchor};

const LAYOUT_COUNT: usize = 9;
const WORLD_BYTES: u32 = 12;
const ENTITY_BYTES: u32 = 36;
const TEMPLATE_BYTES: u32 = 40;

#[derive(Clone, Copy, Debug)]
pub(in crate::engine::chunk_residency) struct RelationUploadLayout {
    offsets: [u32; LAYOUT_COUNT],
    counts: [u32; LAYOUT_COUNT],
    active_mask: u32,
    remap_relative_offset: u32,
}

impl RelationUploadLayout {
    pub(in crate::engine::chunk_residency) const fn uniform_stride(self) -> u32 {
        if self.active_mask.is_power_of_two() {
            layout_stride(self.active_mask.trailing_zeros() as usize)
        } else {
            0
        }
    }
}

pub(in crate::engine::chunk_residency) struct RelationUpload<'a> {
    rows: &'a [PagedRelation],
    layout: RelationUploadLayout,
    total_bytes: u64,
}

impl<'a> RelationUpload<'a> {
    pub(in crate::engine::chunk_residency) fn new(
        rows: &'a [PagedRelation],
    ) -> Result<Self, ChunkResidencyError> {
        let mut counts = [0_u32; LAYOUT_COUNT];
        for row in rows {
            let layout = layout_index(row);
            counts[layout] = counts[layout]
                .checked_add(1)
                .ok_or(ChunkResidencyError::LocalAddressOverflow)?;
        }
        let mut offsets = [0_u32; LAYOUT_COUNT];
        let mut active_mask = 0_u32;
        let mut offset = 0_u64;
        for (layout, rows) in counts.into_iter().enumerate() {
            if rows == 0 {
                continue;
            }
            offsets[layout] =
                u32::try_from(offset).map_err(|_| ChunkResidencyError::LocalAddressOverflow)?;
            active_mask |= 1_u32 << layout;
            let bytes = u64::from(rows)
                .checked_mul(u64::from(layout_stride(layout)))
                .ok_or(ChunkResidencyError::SizeOverflow)?;
            offset = offset
                .checked_add(bytes)
                .ok_or(ChunkResidencyError::SizeOverflow)?;
        }
        let remap_relative_offset = if active_mask.count_ones() > 1 {
            let remap =
                u32::try_from(offset).map_err(|_| ChunkResidencyError::LocalAddressOverflow)?;
            let bytes = u64::try_from(rows.len())
                .map_err(|_| ChunkResidencyError::SizeOverflow)?
                .checked_mul(4)
                .ok_or(ChunkResidencyError::SizeOverflow)?;
            offset = offset
                .checked_add(bytes)
                .ok_or(ChunkResidencyError::SizeOverflow)?;
            remap
        } else {
            u32::MAX
        };
        Ok(Self {
            rows,
            layout: RelationUploadLayout {
                offsets,
                counts,
                active_mask,
                remap_relative_offset,
            },
            total_bytes: offset,
        })
    }

    pub(in crate::engine::chunk_residency) const fn byte_len(&self) -> u64 {
        self.total_bytes
    }

    pub(in crate::engine::chunk_residency) const fn layout(&self) -> RelationUploadLayout {
        self.layout
    }

    pub(in crate::engine::chunk_residency) fn write(
        &self,
        target: &mut [u8],
    ) -> Result<(), ChunkResidencyError> {
        let bases = logical_bases(self.layout.counts)?;
        let mut cursors = [0_u32; LAYOUT_COUNT];
        for (logical_row, row) in self.rows.iter().copied().enumerate() {
            let layout = layout_index(&row);
            let row_offset = u64::from(self.layout.offsets[layout])
                .checked_add(
                    u64::from(cursors[layout])
                        .checked_mul(u64::from(layout_stride(layout)))
                        .ok_or(ChunkResidencyError::SizeOverflow)?,
                )
                .ok_or(ChunkResidencyError::SizeOverflow)?;
            write_row(target, row_offset, row)?;
            if self.layout.remap_relative_offset != u32::MAX {
                let compact = bases[layout]
                    .checked_add(cursors[layout])
                    .ok_or(ChunkResidencyError::LocalAddressOverflow)?;
                let offset = u64::from(self.layout.remap_relative_offset)
                    .checked_add(
                        u64::from(compact)
                            .checked_mul(4)
                            .ok_or(ChunkResidencyError::SizeOverflow)?,
                    )
                    .ok_or(ChunkResidencyError::SizeOverflow)?;
                write_bytes(
                    target,
                    offset,
                    &u32::try_from(logical_row)
                        .map_err(|_| ChunkResidencyError::LocalAddressOverflow)?
                        .to_le_bytes(),
                )?;
            }
            cursors[layout] += 1;
        }
        Ok(())
    }
}

fn logical_bases(counts: [u32; LAYOUT_COUNT]) -> Result<[u32; LAYOUT_COUNT], ChunkResidencyError> {
    let mut bases = [0_u32; LAYOUT_COUNT];
    let mut base = 0_u32;
    for (layout, rows) in counts.into_iter().enumerate() {
        bases[layout] = base;
        base = base
            .checked_add(rows)
            .ok_or(ChunkResidencyError::LocalAddressOverflow)?;
    }
    Ok(bases)
}

fn layout_index(row: &PagedRelation) -> usize {
    anchor_layout(row.start) * 3 + anchor_layout(row.end)
}

const fn anchor_layout(anchor: PagedSpatialAnchor) -> usize {
    match anchor {
        PagedSpatialAnchor::World(_) => 0,
        PagedSpatialAnchor::Entity(_) => 1,
        PagedSpatialAnchor::TemplatePart(_) => 2,
    }
}

const fn layout_stride(layout: usize) -> u32 {
    anchor_stride(layout / 3) + anchor_stride(layout % 3)
}

const fn anchor_stride(layout: usize) -> u32 {
    match layout {
        0 => WORLD_BYTES,
        1 => ENTITY_BYTES,
        _ => TEMPLATE_BYTES,
    }
}

fn write_row(
    target: &mut [u8],
    offset: u64,
    row: PagedRelation,
) -> Result<(), ChunkResidencyError> {
    let next = write_anchor(target, offset, row.start)?;
    let _ = write_anchor(target, next, row.end)?;
    Ok(())
}

fn write_anchor(
    target: &mut [u8],
    offset: u64,
    anchor: PagedSpatialAnchor,
) -> Result<u64, ChunkResidencyError> {
    match anchor {
        PagedSpatialAnchor::World(position) => {
            write_bytes(
                target,
                offset,
                bytemuck::cast_slice(&[position.x, position.y, position.z]),
            )?;
            add(offset, WORLD_BYTES)
        }
        PagedSpatialAnchor::Entity(reference) => {
            write_reference(target, offset, reference)?;
            add(offset, ENTITY_BYTES)
        }
        PagedSpatialAnchor::TemplatePart(reference) => {
            write_reference(target, offset, reference.instance())?;
            let part_offset = add(offset, ENTITY_BYTES)?;
            write_bytes(target, part_offset, &reference.part_row().to_le_bytes())?;
            add(offset, TEMPLATE_BYTES)
        }
    }
}

fn write_reference(
    target: &mut [u8],
    offset: u64,
    reference: ChunkEntityRef,
) -> Result<(), ChunkResidencyError> {
    let mut cursor = offset;
    for bytes in [
        reference.dataset().get().to_le_bytes(),
        reference.chunk().get().to_le_bytes(),
        reference.occurrence().get().to_le_bytes(),
        reference.row().get().to_le_bytes(),
    ] {
        write_bytes(target, cursor, &bytes)?;
        cursor = add(cursor, 8)?;
    }
    write_bytes(target, cursor, &(reference.kind() as u32).to_le_bytes())
}

fn write_bytes(target: &mut [u8], offset: u64, bytes: &[u8]) -> Result<(), ChunkResidencyError> {
    let start = usize::try_from(offset).map_err(|_| ChunkResidencyError::SizeOverflow)?;
    let end = start
        .checked_add(bytes.len())
        .ok_or(ChunkResidencyError::SizeOverflow)?;
    let Some(range) = target.get_mut(start..end) else {
        return Err(ChunkResidencyError::SizeOverflow);
    };
    range.copy_from_slice(bytes);
    Ok(())
}

fn add(offset: u64, bytes: u32) -> Result<u64, ChunkResidencyError> {
    offset
        .checked_add(u64::from(bytes))
        .ok_or(ChunkResidencyError::SizeOverflow)
}
