//! Arena synchronization and per-column GPU writes.
use crate::DerivedCache;
use crate::error::RenderError;
use molgfx_core::{RowDomain, Scene, VisualAttributeRef};
use molgfx_gpu::{Device, Queue};

use super::{MISSING_CHUNK, MISSING_CHUNK_LEN, PropertyColumn, VisualPropertyTable};

impl<D: Device> VisualPropertyTable<D> {
    pub(in crate::scene_gpu) fn sync(
        &mut self,
        device: &D,
        queue: &D::Queue,
        scene: &Scene,
        timeline_layout: &D::BindGroupLayout,
        derived_cache: &mut DerivedCache,
        frame: u64,
    ) -> Result<bool, RenderError> {
        let source_key = (
            scene.cache_identity(),
            scene.representation_revision(),
            scene.domain_visual_revision(),
            scene.generic_timeline_binding_revision(),
            scene.structure_revision(),
        );
        let source_changed = self.source_key != Some(source_key);
        let scene_changed = self
            .source_key
            .is_some_and(|(identity, _, _, _, _)| identity != source_key.0);
        let evicted = self.columns.iter().any(|column| {
            column
                .cache_key
                .is_some_and(|key| !derived_cache.contains(key))
        });
        let property_plan_changed = (source_changed || evicted)
            && self.refresh_plan(scene, scene_changed || evicted, derived_cache, frame)?;
        let state_plan_changed =
            source_changed && self.refresh_state_plan(scene, scene_changed || evicted)?;
        let plan_changed = property_plan_changed || state_plan_changed;
        self.source_key = Some(source_key);
        let bytes = self.required_bytes();
        let rebound = self
            .buffer
            .reserve(device, "visual property table", bytes)?;
        if rebound {
            self.binding_revision = self.binding_revision.wrapping_add(1);
        }
        let Some(buffer) = self.buffer.get() else {
            return Ok(false);
        };
        if plan_changed || rebound {
            queue.write_buffer(buffer, 0, bytemuck::bytes_of(&f32::NAN));
            queue.write_buffer(buffer, 4, bytemuck::bytes_of(&0_u32));
        }
        let force_upload = plan_changed || rebound;
        let mut uploaded = force_upload;
        for column in &mut self.columns {
            if let VisualAttributeRef::Attribute { handle, .. } = column.reference
                && column.temporal
                && let Some((start, end, alpha, revision)) = scene.attribute_frames(handle)
            {
                if force_upload {
                    upload_temporal_column::<D>(queue, buffer, column, start, end, alpha);
                } else if column.timeline_revision != revision {
                    queue.write_buffer(
                        buffer,
                        u64::from(column.offset.saturating_sub(1)) * 4,
                        bytemuck::bytes_of(&alpha),
                    );
                }
                column.timeline_revision = revision;
                uploaded = true;
                continue;
            }
            let (revision, dirty_rows) = match column.reference {
                VisualAttributeRef::Attribute { handle, .. } => {
                    crate::fallback(scene.attribute_change(handle), (0, 0..column.length))
                }
                VisualAttributeRef::LegacyScalar(handle) => {
                    let revision = crate::fallback(scene.property_content_revision(handle), 0);
                    (revision, 0..column.length)
                }
                VisualAttributeRef::Column { .. } => (0, 0..column.length),
            };
            if !force_upload && revision == column.revision {
                continue;
            }
            let dirty_rows = if force_upload {
                0..column.length
            } else {
                dirty_rows
            };
            upload_column::<D>(queue, buffer, scene, column, dirty_rows);
            column.revision = revision;
            uploaded = true;
        }
        for column in &mut self.states {
            let Some(state) = scene.interaction_state(column.structure) else {
                continue;
            };
            let revision = state.revision().get();
            if !force_upload && revision == column.revision {
                continue;
            }
            queue.write_buffer(buffer, u64::from(column.offset) * 4, state.as_bytes());
            column.revision = revision;
            uploaded = true;
        }
        if plan_changed || rebound {
            self.bind_timelines(device, queue, timeline_layout)?;
        }
        Ok(uploaded)
    }
}

pub(super) fn upload_temporal_column<D: Device>(
    queue: &D::Queue,
    buffer: &D::Buffer,
    column: &PropertyColumn,
    start: &molgfx_core::AttributeValues,
    end: &molgfx_core::AttributeValues,
    alpha: f32,
) {
    let end_offset = column
        .offset
        .saturating_add(column.length.saturating_mul(column.stride_words));
    queue.write_buffer(
        buffer,
        u64::from(column.offset.saturating_sub(2)) * 4,
        bytemuck::bytes_of(&end_offset),
    );
    queue.write_buffer(
        buffer,
        u64::from(column.offset.saturating_sub(1)) * 4,
        bytemuck::bytes_of(&alpha),
    );
    queue.write_buffer(buffer, u64::from(column.offset) * 4, start.as_bytes());
    queue.write_buffer(buffer, u64::from(end_offset) * 4, end.as_bytes());
}

pub(super) fn write_missing<D: Device>(
    queue: &D::Queue,
    buffer: &D::Buffer,
    offset: u64,
    length: u32,
) {
    let mut remaining = length;
    let mut byte_offset = offset;
    while remaining > 0 {
        let count_u32 = remaining.min(MISSING_CHUNK_LEN);
        let count = match usize::try_from(count_u32) {
            Ok(count) => count,
            Err(_) => MISSING_CHUNK.len(),
        };
        queue.write_buffer(
            buffer,
            byte_offset,
            bytemuck::cast_slice(&MISSING_CHUNK[..count]),
        );
        remaining -= count_u32;
        byte_offset += u64::from(count_u32) * std::mem::size_of::<f32>() as u64;
    }
}

pub(super) fn column_shape(scene: &Scene, reference: VisualAttributeRef) -> Option<(usize, u32)> {
    match reference {
        VisualAttributeRef::Attribute { handle, kind } => {
            let attribute = scene.attribute(handle)?;
            (attribute.kind() == kind).then_some((attribute.len(), kind.stride() / 4))
        }
        VisualAttributeRef::LegacyScalar(handle) => {
            Some((scene.atom_property(handle)?.values().len(), 1))
        }
        VisualAttributeRef::Column { .. } => None,
    }
}

pub(super) fn reference_matches_domain(
    scene: &Scene,
    domain: RowDomain,
    reference: VisualAttributeRef,
) -> bool {
    match reference {
        VisualAttributeRef::Attribute { handle, kind } => scene
            .attribute_for_domain(handle, domain)
            .is_some_and(|attribute| attribute.kind() == kind),
        VisualAttributeRef::LegacyScalar(handle) => match domain {
            RowDomain::Atoms(structure) => {
                scene.property_for_structure(handle, structure).is_some()
            }
            _ => false,
        },
        VisualAttributeRef::Column { .. } => false,
    }
}

pub(super) fn upload_column<D: Device>(
    queue: &D::Queue,
    buffer: &D::Buffer,
    scene: &Scene,
    column: &PropertyColumn,
    dirty_rows: std::ops::Range<u32>,
) {
    let byte_offset = u64::from(
        column
            .offset
            .saturating_add(dirty_rows.start.saturating_mul(column.stride_words)),
    ) * 4;
    match column.reference {
        VisualAttributeRef::Attribute { handle, .. } => {
            let Some(attribute) = scene.attribute(handle) else {
                write_missing::<D>(
                    queue,
                    buffer,
                    byte_offset,
                    dirty_rows
                        .end
                        .saturating_sub(dirty_rows.start)
                        .saturating_mul(column.stride_words),
                );
                return;
            };
            let bytes = attribute.values().as_bytes();
            let stride = attribute.stride() as usize;
            let start = dirty_rows.start as usize * stride;
            let end = dirty_rows.end as usize * stride;
            if let Some(changed) = bytes.get(start..end) {
                queue.write_buffer(buffer, byte_offset, changed);
            }
        }
        VisualAttributeRef::LegacyScalar(handle) => {
            if let Some(property) = scene.atom_property(handle) {
                queue.write_buffer(buffer, byte_offset, bytemuck::cast_slice(property.values()));
            } else {
                write_missing::<D>(queue, buffer, byte_offset, column.length);
            }
        }
        VisualAttributeRef::Column { .. } => {
            write_missing::<D>(queue, buffer, byte_offset, column.length);
        }
    }
}
