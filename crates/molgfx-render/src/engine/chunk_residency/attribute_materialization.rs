//! Optional in-arena materialization for reused paged attribute timelines.

use super::ChunkGpuResidency;
use crate::engine::chunk_draw_plan::ResidentAttributeMaterialization;
use crate::engine::{ChunkResidencyError, DerivedCacheKey};
use crate::{DerivedCache, DerivedFootprint};
use molgfx_core::{AttributeKind, ResidencyTicket};
use molgfx_gpu::Device;

impl<D: Device> ChunkGpuResidency<D> {
    pub(super) fn plan_attribute_materializations(
        &mut self,
        cache: &mut DerivedCache,
        frame: u64,
    ) -> Result<(), ChunkResidencyError> {
        let mut topology_changed = false;
        for window in &self.attribute_windows {
            let Some(source) = self.resident_attribute_column(window.attribute) else {
                cache.release(DerivedCacheKey::PagedAttribute(window.attribute));
                continue;
            };
            let key = DerivedCacheKey::PagedAttribute(window.attribute);
            if !supports_interpolation(source.kind)
                || self.attribute_consumers(window.attribute) < 2
            {
                cache.release(key);
                continue;
            }
            let _plan = cache.plan(
                key,
                DerivedFootprint {
                    cpu_bytes: 0,
                    gpu_bytes: source.byte_len,
                },
                self.attribute_consumers(window.attribute),
                frame,
            );
        }

        let mut index = 0;
        while index < self.attribute_materializations.len() {
            let ticket = self.attribute_materializations[index].ticket;
            let source_missing = self.materialization_source(ticket).is_none();
            if !cache.contains(DerivedCacheKey::PagedAttribute(ticket)) || source_missing {
                let _removed = self.attribute_materializations.swap_remove(index);
                let allocation = self
                    .attribute_materialization_allocations
                    .swap_remove(index);
                self.arena.release(allocation)?;
                topology_changed = true;
                if source_missing {
                    cache.release(DerivedCacheKey::PagedAttribute(ticket));
                }
            } else {
                index += 1;
            }
        }

        for window_index in 0..self.attribute_windows.len() {
            let window = self.attribute_windows[window_index];
            let key = DerivedCacheKey::PagedAttribute(window.attribute);
            if !cache.contains(key) {
                continue;
            }
            let Some((start, end, byte_len, _kind)) = self.materialization_source(window.attribute)
            else {
                continue;
            };
            if let Some(entry) = self
                .attribute_materializations
                .iter_mut()
                .find(|entry| entry.ticket == window.attribute)
            {
                entry.start_byte_offset = start;
                entry.end_byte_offset = end;
                entry.interpolation = window.interpolation;
                continue;
            }
            if self.attribute_materializations.len() == self.attribute_materializations.capacity() {
                cache.release(key);
                continue;
            }
            let Ok(allocation) = self.arena.allocate(byte_len) else {
                cache.release(key);
                continue;
            };
            let word_count = u32::try_from(byte_len / 4)
                .map_err(|_| ChunkResidencyError::LocalAddressOverflow)?;
            self.attribute_materializations
                .push(ResidentAttributeMaterialization {
                    ticket: window.attribute,
                    start_byte_offset: start,
                    end_byte_offset: end,
                    output_byte_offset: allocation.byte_offset(),
                    word_count,
                    interpolation: window.interpolation,
                });
            self.attribute_materialization_allocations.push(allocation);
            topology_changed = true;
        }
        sort_materializations(
            &mut self.attribute_materializations,
            &mut self.attribute_materialization_allocations,
        );
        if topology_changed {
            self.attribute_timeline_revision =
                self.attribute_timeline_revision.wrapping_add(1).max(1);
        }
        Ok(())
    }

    pub(super) fn attribute_materialization_plans(&self) -> &[ResidentAttributeMaterialization] {
        &self.attribute_materializations
    }

    pub(super) fn attribute_materialization(
        &self,
        ticket: ResidencyTicket,
    ) -> Option<&ResidentAttributeMaterialization> {
        self.attribute_materializations
            .binary_search_by_key(&ticket, |entry| entry.ticket)
            .ok()
            .and_then(|index| self.attribute_materializations.get(index))
    }

    fn materialization_source(
        &self,
        ticket: ResidencyTicket,
    ) -> Option<(u64, u64, u64, AttributeKind)> {
        let window = self
            .attribute_windows
            .binary_search_by_key(&ticket, |window| window.attribute)
            .ok()
            .and_then(|index| self.attribute_windows.get(index))?;
        let source = self.resident_attribute_column(ticket)?;
        let start = self.resident_attribute_column(window.start)?;
        let end = self.resident_attribute_column(window.end)?;
        (start.target == source.target
            && end.target == source.target
            && start.kind == source.kind
            && end.kind == source.kind
            && start.local_rows == source.local_rows
            && end.local_rows == source.local_rows)
            .then_some((
                start.byte_offset,
                end.byte_offset,
                source.byte_len,
                source.kind,
            ))
    }

    fn attribute_consumers(&self, ticket: ResidencyTicket) -> u32 {
        self.relation_plan
            .iter()
            .filter_map(|plan| plan.visual.as_ref())
            .map(|visual| {
                visual
                    .bindings()
                    .iter()
                    .filter(|binding| binding.ticket() == ticket)
                    .count()
            })
            .fold(0_u32, |count, value| {
                count.saturating_add(match u32::try_from(value) {
                    Ok(value) => value,
                    Err(_) => u32::MAX,
                })
            })
    }
}

fn sort_materializations(
    plans: &mut [ResidentAttributeMaterialization],
    allocations: &mut [molgfx_gpu::ArenaAllocation],
) {
    for index in 1..plans.len() {
        let mut cursor = index;
        while cursor > 0 && plans[cursor].ticket < plans[cursor - 1].ticket {
            plans.swap(cursor, cursor - 1);
            allocations.swap(cursor, cursor - 1);
            cursor -= 1;
        }
    }
}

const fn supports_interpolation(kind: AttributeKind) -> bool {
    matches!(kind, AttributeKind::Scalar | AttributeKind::Vector)
}
