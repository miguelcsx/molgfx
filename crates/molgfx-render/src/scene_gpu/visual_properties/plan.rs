//! Column layout planning for properties and interaction state.
use crate::engine::DerivedCacheKey;
use crate::error::RenderError;
use crate::{DerivedCache, DerivedFootprint, MaterializationPlan};
use molgfx_core::{Scene, VisualAttributeRef};
use molgfx_gpu::Device;

use super::timeline::reference_consumers;
use super::upload::column_shape;
use super::{AttributeTimelineDispatch, PropertyColumn, StateColumn, VisualPropertyTable};

impl<D: Device> VisualPropertyTable<D> {
    pub(super) fn refresh_plan(
        &mut self,
        scene: &Scene,
        force: bool,
        derived_cache: &mut DerivedCache,
        frame: u64,
    ) -> Result<bool, RenderError> {
        self.handle_scratch.clear();
        for (_, representation) in scene.representations() {
            // A colour scheme reads one scalar column; it is planned here so
            // the shader can sample it from the same arena visual programs use.
            if let Some(handle) = representation.color.property_handle() {
                self.handle_scratch
                    .push(VisualAttributeRef::LegacyScalar(handle));
            }
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
        let mut offset = 2_u32;
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

    pub(super) fn required_bytes(&self) -> u64 {
        let words = self.states.last().map_or_else(
            || self.property_word_count(),
            |column| column.offset.saturating_add(column.length),
        );
        u64::from(words) * 4
    }

    pub(super) fn property_word_count(&self) -> u32 {
        self.columns.last().map_or(2, |column| {
            column
                .offset
                .saturating_sub(u32::from(column.temporal) * 2)
                .saturating_add(column.storage_words)
        })
    }

    pub(super) fn refresh_state_plan(
        &mut self,
        scene: &Scene,
        force: bool,
    ) -> Result<bool, RenderError> {
        let stated = scene
            .structures()
            .filter(|(structure, _)| scene.interaction_state(*structure).is_some())
            .count();
        let unchanged = !force
            && self.states.len() == stated
            && self.states.iter().all(|column| {
                scene
                    .interaction_state(column.structure)
                    .is_some_and(|state| state.len() == column.length as usize)
            });
        if unchanged {
            return Ok(false);
        }
        self.states.clear();
        let mut offset = self.property_word_count();
        for (structure, _) in scene.structures() {
            // A structure without a state column simply has no interaction bits.
            // Programs reading it address the missing-value slot, which is zero.
            let Some(state) = scene.interaction_state(structure) else {
                continue;
            };
            let length =
                u32::try_from(state.len()).map_err(|_| molgfx_gpu::GpuError::LimitExceeded {
                    resource: "visual interaction state table",
                    limit: u64::from(u32::MAX) * 4,
                })?;
            self.states.push(StateColumn {
                structure,
                offset,
                length,
                revision: 0,
            });
            offset =
                offset
                    .checked_add(length)
                    .ok_or_else(|| molgfx_gpu::GpuError::LimitExceeded {
                        resource: "visual interaction state table",
                        limit: u64::from(u32::MAX) * 4,
                    })?;
        }
        Ok(true)
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
