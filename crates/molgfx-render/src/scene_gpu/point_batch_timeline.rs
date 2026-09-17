// Optional GPU materialization for deformable point streams.

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct PointTimelineConfig {
    values: [f32; 4],
    counts: [u32; 4],
}

type PointMaterializationResources<D> = (
    Option<<D as Device>::Buffer>,
    Option<<D as Device>::Buffer>,
    Option<<D as Device>::BindGroup>,
);

#[derive(Clone, Copy)]
pub(crate) struct GenericPointTimelineDispatch<'a, D: Device> {
    pub(crate) group: &'a D::BindGroup,
    pub(crate) groups: [u32; 2],
}

impl<D: Device> GpuPointBatches<D> {
    fn plan_timelines(context: &mut PointBatchSync<'_, D>) {
        for (handle, batch) in context
            .scene
            .point_batches()
            .filter(|(_, batch)| batch.visible())
        {
            let key = point_timeline_cache_key(handle);
            if context.scene.point_frames(handle).is_none() {
                context.derived_cache.release(key);
                continue;
            }
            let footprint = DerivedFootprint {
                cpu_bytes: 0,
                gpu_bytes: u64::from(batch.source_rows().len()).saturating_mul(12),
            };
            let _plan = context.derived_cache.plan(key, footprint, 2, context.frame);
        }
        let _evicted_count = context.derived_cache.take_evictions().count();
    }
}

impl<D: Device> GpuPointBatch<D> {
    fn active_start_buffer(&self) -> Option<&D::Buffer> {
        if let Some(materialized) = &self.materialized {
            return Some(materialized);
        }
        if let Some(start) = &self.timeline_start {
            return Some(start);
        }
        self.positions.get()
    }

    fn active_end_buffer(&self) -> Option<&D::Buffer> {
        if let Some(materialized) = &self.materialized {
            return Some(materialized);
        }
        if let Some(end) = &self.timeline_end {
            return Some(end);
        }
        self.positions.get()
    }

    fn sync_timeline(&mut self, context: &PointBatchSync<'_, D>) -> Result<bool, RenderError> {
        let Some((start, end, alpha, _)) = context.scene.point_frames(self.handle) else {
            let changed = self.timeline_start.take().is_some()
                || self.timeline_end.take().is_some()
                || self.materialized.take().is_some();
            self.timeline_source = None;
            self.timeline_config = None;
            self.timeline_group = None;
            return Ok(changed);
        };
        let identity = frame_identity(start, end);
        let source_changed = self.timeline_source != Some(identity);
        if source_changed {
            self.timeline_start = Some(storage_buffer(
                context.device,
                context.queue,
                "generic point timeline start",
                start.as_ref(),
            )?);
            self.timeline_end = Some(storage_buffer(
                context.device,
                context.queue,
                "generic point timeline end",
                end.as_ref(),
            )?);
            self.timeline_source = Some(identity);
        }
        let should_materialize = timeline_materialized(context, self.handle);
        let mode_changed = should_materialize != self.materialized.is_some();
        if should_materialize && (self.materialized.is_none() || source_changed) {
            (self.materialized, self.timeline_config, self.timeline_group) = create_materialization(
                context,
                self.timeline_start.as_ref(),
                self.timeline_end.as_ref(),
                alpha,
                self.count,
            )?;
        } else if should_materialize {
            let Some(config) = &self.timeline_config else {
                return Ok(source_changed || mode_changed);
            };
            write_timeline_config::<D>(context.queue, config, alpha, self.count);
        } else {
            self.materialized = None;
            self.timeline_config = None;
            self.timeline_group = None;
        }
        Ok(source_changed || mode_changed)
    }
}

fn timeline_materialized<D: Device>(
    context: &PointBatchSync<'_, D>,
    handle: PointBatchHandle,
) -> bool {
    context
        .derived_cache
        .contains(point_timeline_cache_key(handle))
}

const fn point_timeline_cache_key(handle: PointBatchHandle) -> crate::engine::DerivedCacheKey {
    crate::engine::DerivedCacheKey::ScenePoint(handle)
}

fn frame_identity(start: &std::sync::Arc<[[f32; 3]]>, end: &std::sync::Arc<[[f32; 3]]>) -> (usize, usize) {
    (start.as_ptr() as usize, end.as_ptr() as usize)
}

fn buffer_or<'a, D: Device>(candidate: Option<&'a D::Buffer>, fallback: &'a D::Buffer) -> &'a D::Buffer {
    match candidate {
        Some(buffer) => buffer,
        None => fallback,
    }
}

fn storage_buffer<D: Device, T: bytemuck::Pod>(
    device: &D,
    queue: &D::Queue,
    label: &'static str,
    rows: &[T],
) -> Result<D::Buffer, RenderError> {
    let bytes = bytemuck::cast_slice(rows);
    let buffer = device.create_buffer(&BufferDesc {
        label,
        size: u64::try_from(bytes.len()).map_err(|_| missing_buffer())?.max(4),
        usage: BufferUsage::STORAGE.union(BufferUsage::COPY_DST),
    })?;
    queue.write_buffer(&buffer, 0, bytes);
    Ok(buffer)
}

fn create_materialization<D: Device>(
    context: &PointBatchSync<'_, D>,
    start: Option<&D::Buffer>,
    end: Option<&D::Buffer>,
    alpha: f32,
    count: u32,
) -> Result<PointMaterializationResources<D>, RenderError> {
    let (Some(start), Some(end)) = (start, end) else {
        return Ok((None, None, None));
    };
    let output = context.device.create_buffer(&BufferDesc {
        label: "materialized generic point timeline",
        size: u64::from(count).saturating_mul(12).max(4),
        usage: BufferUsage::STORAGE,
    })?;
    let config = context.device.create_buffer(&BufferDesc {
        label: "generic point timeline configuration",
        size: std::mem::size_of::<PointTimelineConfig>() as u64,
        usage: BufferUsage::UNIFORM.union(BufferUsage::COPY_DST),
    })?;
    write_timeline_config::<D>(context.queue, &config, alpha, count);
    let group = context.device.create_bind_group(&BindGroupDesc {
        label: "group0: generic point timeline",
        layout: context.timeline_layout,
        entries: &[
            BindGroupEntry::Buffer { binding: 0, buffer: start },
            BindGroupEntry::Buffer { binding: 1, buffer: end },
            BindGroupEntry::Buffer { binding: 2, buffer: &output },
            BindGroupEntry::Buffer { binding: 3, buffer: &config },
        ],
    });
    Ok((Some(output), Some(config), Some(group)))
}

fn write_timeline_config<D: Device>(queue: &D::Queue, buffer: &D::Buffer, alpha: f32, count: u32) {
    queue.write_buffer(
        buffer,
        0,
        bytemuck::bytes_of(&PointTimelineConfig {
            values: [alpha, 0.0, 0.0, 0.0],
            counts: [count, 0, 0, 0],
        }),
    );
}
