//! Relation-source views for direct or materialized rigid timelines.

use super::{BindGroupEntry, Device, GpuInstanceBatches, InstanceBatchHandle, buffer_entry};

impl<D: Device> GpuInstanceBatches<D> {
    pub(in crate::scene_gpu) fn relation_source_entries(
        &self,
        handle: InstanceBatchHandle,
        bindings: [u32; 3],
    ) -> Option<[BindGroupEntry<'_, D>; 3]> {
        let index = self
            .batches
            .binary_search_by_key(&handle, |entry| entry.handle)
            .ok()?;
        let batch = &self.batches[index];
        let (start, end) = match (
            batch.materialized.as_ref(),
            batch.timeline_start.as_ref(),
            batch.timeline_end.as_ref(),
        ) {
            (Some(materialized), _, _) => (materialized, materialized),
            (None, Some(start), Some(end)) => (start, end),
            _ => (&batch.transforms, &batch.transforms),
        };
        Some([
            buffer_entry(bindings[0], start),
            buffer_entry(bindings[1], end),
            buffer_entry(bindings[2], &batch.config),
        ])
    }
}
