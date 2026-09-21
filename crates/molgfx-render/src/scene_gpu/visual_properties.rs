//! Scene-wide `array<u32>` arena for typed columns referenced by visual programs.
//!
//! A property is uploaded once even when several representations consume it.
//! The table is rebuilt only when visual bindings change; content revisions
//! rewrite the existing column directly from the caller-owned slice without a
//! second host-side copy.

use super::grow_buffer::GrowBuffer;
use crate::engine::DerivedCacheKey;
use crate::error::RenderError;
use crate::{DerivedCache, DerivedFootprint, MaterializationPlan};
use molgfx_core::{RowDomain, Scene, StructureHandle, VisualAttributeRef, VisualStyle};
use molgfx_gpu::{BindGroupDesc, BindGroupEntry, BufferDesc, BufferUsage, Device, Queue};

const MISSING_CHUNK: [u32; 256] = [f32::NAN.to_bits(); 256];
const MISSING_CHUNK_LEN: u32 = 256;

#[derive(Clone, Copy, Debug)]
struct PropertyColumn {
    reference: VisualAttributeRef,
    offset: u32,
    length: u32,
    stride_words: u32,
    revision: u64,
    timeline_revision: u64,
    temporal: bool,
    storage_words: u32,
    materialized_offset: Option<u32>,
    cache_key: Option<DerivedCacheKey>,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct AttributeTimelineConfig {
    offsets: [u32; 4],
    counts: [u32; 4],
}

#[derive(Debug)]
struct AttributeTimelineGpu<D: Device> {
    config: D::Buffer,
    group: D::BindGroup,
    groups: [u32; 2],
    paged_plan: Option<crate::engine::chunk_draw_plan::ResidentAttributeMaterialization>,
}

#[derive(Clone, Copy)]
pub(crate) struct AttributeTimelineDispatch<'a, D: Device> {
    pub(crate) group: &'a D::BindGroup,
    pub(crate) groups: [u32; 2],
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct AttributeArenaBinding {
    pub(super) offsets: [u32; 4],
    pub(super) layouts: [u32; 4],
}

/// One persistent, deduplicated column-major property table per scene.
#[derive(Debug)]
pub(super) struct VisualPropertyTable<D: Device> {
    buffer: GrowBuffer<D>,
    columns: Vec<PropertyColumn>,
    planned_handles: Vec<VisualAttributeRef>,
    handle_scratch: Vec<VisualAttributeRef>,
    source_key: Option<(u64, u64, u64, u64)>,
    binding_revision: u64,
    timelines: Vec<AttributeTimelineGpu<D>>,
    paged_timelines: Vec<AttributeTimelineGpu<D>>,
    paged_source_revision: u64,
}

impl<D: Device> VisualPropertyTable<D> {
    pub(super) fn new() -> Self {
        Self {
            buffer: GrowBuffer::new(),
            columns: Vec::new(),
            planned_handles: Vec::new(),
            handle_scratch: Vec::new(),
            source_key: None,
            binding_revision: 0,
            timelines: Vec::new(),
            paged_timelines: Vec::new(),
            paged_source_revision: 0,
        }
    }

    pub(super) fn buffer(&self) -> Option<&D::Buffer> {
        self.buffer.get()
    }

    pub(super) const fn binding_revision(&self) -> u64 {
        self.binding_revision
    }

    pub(super) fn sync(
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
        );
        let source_changed = self.source_key != Some(source_key);
        let scene_changed = self
            .source_key
            .is_some_and(|(identity, _, _, _)| identity != source_key.0);
        let evicted = self.columns.iter().any(|column| {
            column
                .cache_key
                .is_some_and(|key| !derived_cache.contains(key))
        });
        let plan_changed = (source_changed || evicted)
            && self.refresh_plan(scene, scene_changed || evicted, derived_cache, frame)?;
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
        if plan_changed || rebound {
            self.bind_timelines(device, queue, timeline_layout)?;
        }
        Ok(uploaded)
    }

    pub(super) fn offsets(
        &self,
        scene: &Scene,
        structure: StructureHandle,
        style: Option<&VisualStyle>,
    ) -> AttributeArenaBinding {
        self.offsets_for_domain(scene, RowDomain::Atoms(structure), style)
    }

    pub(super) fn offsets_for_domain(
        &self,
        scene: &Scene,
        domain: RowDomain,
        style: Option<&VisualStyle>,
    ) -> AttributeArenaBinding {
        let mut binding = AttributeArenaBinding::default();
        let Some(style) = style else { return binding };
        for (slot, reference) in style.program().attributes().iter().enumerate() {
            if slot == binding.offsets.len() {
                break;
            }
            if !reference_matches_domain(scene, domain, *reference) {
                continue;
            }
            let Ok(index) = self
                .columns
                .binary_search_by_key(reference, |column| column.reference)
            else {
                continue;
            };
            let column = self.columns[index];
            binding.offsets[slot] = crate::fallback(column.materialized_offset, column.offset);
            binding.layouts[slot] = column.stride_words
                | ((column.reference.kind() as u32) << 8)
                | (u32::from(column.temporal && column.materialized_offset.is_none()) << 16);
        }
        binding
    }

    fn refresh_plan(
        &mut self,
        scene: &Scene,
        force: bool,
        derived_cache: &mut DerivedCache,
        frame: u64,
    ) -> Result<bool, RenderError> {
        self.handle_scratch.clear();
        for (_, representation) in scene.representations() {
            let Some(style) = &representation.visual else {
                continue;
            };
            self.handle_scratch
                .extend_from_slice(style.program().attributes());
        }
        for (_, descriptor) in scene.domain_visuals() {
            self.handle_scratch
                .extend_from_slice(descriptor.style().program().attributes());
        }
        self.handle_scratch.sort_unstable();
        self.handle_scratch.dedup();
        if !force && self.handle_scratch == self.planned_handles {
            return Ok(false);
        }
        std::mem::swap(&mut self.handle_scratch, &mut self.planned_handles);
        let old_keys = self
            .columns
            .iter()
            .filter_map(|column| column.cache_key)
            .collect::<Vec<_>>();
        self.columns.clear();
        let mut offset = 1_u32;
        for &reference in &self.planned_handles {
            let Some((length, stride_words)) = column_shape(scene, reference) else {
                continue;
            };
            let length =
                u32::try_from(length).map_err(|_| molgfx_gpu::GpuError::LimitExceeded {
                    resource: "visual property table",
                    limit: u64::from(u32::MAX) * 4,
                })?;
            let temporal = match reference {
                VisualAttributeRef::Attribute { handle, .. } => {
                    scene.attribute_frames(handle).is_some()
                }
                VisualAttributeRef::LegacyScalar(_) | VisualAttributeRef::Column { .. } => false,
            };
            let values_words = length.saturating_mul(stride_words);
            let cache_key = match reference {
                VisualAttributeRef::Attribute { handle, .. } if temporal => {
                    Some(DerivedCacheKey::SceneAttribute(handle))
                }
                _ => None,
            };
            let materialized = cache_key.is_some_and(|key| {
                derived_cache.plan(
                    key,
                    DerivedFootprint {
                        cpu_bytes: 0,
                        gpu_bytes: u64::from(values_words) * 4,
                    },
                    reference_consumers(scene, reference),
                    frame,
                ) == MaterializationPlan::Materialized
            });
            let storage_words = if temporal {
                values_words.saturating_mul(2).saturating_add(2)
            } else {
                values_words
            }
            .saturating_add(u32::from(materialized).saturating_mul(values_words));
            let data_offset = offset.saturating_add(u32::from(temporal) * 2);
            let materialized_offset =
                materialized.then(|| data_offset.saturating_add(values_words.saturating_mul(2)));
            self.columns.push(PropertyColumn {
                reference,
                offset: data_offset,
                length,
                stride_words,
                revision: 0,
                timeline_revision: 0,
                temporal,
                storage_words,
                materialized_offset,
                cache_key: materialized.then_some(cache_key).flatten(),
            });
            offset = offset.checked_add(storage_words).ok_or_else(|| {
                molgfx_gpu::GpuError::LimitExceeded {
                    resource: "visual property table",
                    limit: u64::from(u32::MAX) * 4,
                }
            })?;
        }
        for key in old_keys {
            if !self
                .columns
                .iter()
                .any(|column| column.cache_key == Some(key))
            {
                derived_cache.release(key);
            }
        }
        Ok(true)
    }

    fn required_bytes(&self) -> u64 {
        self.columns.last().map_or(4, |column| {
            u64::from(
                column
                    .offset
                    .saturating_sub(u32::from(column.temporal) * 2)
                    .saturating_add(column.storage_words),
            ) * 4
        })
    }

    pub(crate) fn timeline_dispatches(
        &self,
    ) -> impl Iterator<Item = AttributeTimelineDispatch<'_, D>> {
        self.timelines
            .iter()
            .chain(&self.paged_timelines)
            .map(|timeline| AttributeTimelineDispatch {
                group: &timeline.group,
                groups: timeline.groups,
            })
    }
}

fn upload_temporal_column<D: Device>(
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

fn write_missing<D: Device>(queue: &D::Queue, buffer: &D::Buffer, offset: u64, length: u32) {
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

include!("visual_property_timeline.rs");

fn column_shape(scene: &Scene, reference: VisualAttributeRef) -> Option<(usize, u32)> {
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

fn reference_matches_domain(
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

fn upload_column<D: Device>(
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
