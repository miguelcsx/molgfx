// Relation-source views for direct or materialized rigid timelines.

impl<D: Device> GpuInstanceBatches<D> {
    pub(super) fn relation_source_entries(
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
            BindGroupEntry::Buffer {
                binding: bindings[0],
                buffer: start,
            },
            BindGroupEntry::Buffer {
                binding: bindings[1],
                buffer: end,
            },
            BindGroupEntry::Buffer {
                binding: bindings[2],
                buffer: &batch.config,
            },
        ])
    }
}
