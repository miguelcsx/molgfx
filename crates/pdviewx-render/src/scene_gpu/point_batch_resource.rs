// Persistent resources for one generic point batch.

impl<D: Device> GpuPointBatch<D> {
    #[allow(clippy::too_many_lines)]
    fn new(
        context: &PointBatchSync<'_, D>,
        handle: PointBatchHandle,
        batch: &PointBatch,
        page: u32,
        tiles: &PointTiles<'_, D>,
    ) -> Result<Self, RenderError> {
        let device = context.device;
        let queue = context.queue;
        let row_count = batch.source_rows().len();
        let mut positions = GrowBuffer::new();
        positions.upload(device, queue, "generic point positions", batch.positions().as_ref())?;
        let output = device.create_buffer(&BufferDesc {
            label: "generic point cull output",
            size: runtime_storage_bytes(
                std::mem::size_of::<DrawIndirectArgs>() as u64,
                row_count,
            ),
            usage: BufferUsage::INDIRECT.union(BufferUsage::STORAGE).union(BufferUsage::COPY_DST),
        })?;
        queue.write_buffer(
            &output,
            0,
            bytemuck::bytes_of(&DrawIndirectArgs {
                vertex_count: 6,
                instance_count: 0,
                first_vertex: 0,
                first_instance: 0,
            }),
        );
        let config = device.create_buffer(&BufferDesc {
            label: "generic point configuration",
            size: std::mem::size_of::<PointBatchGpu>() as u64,
            usage: BufferUsage::UNIFORM.union(BufferUsage::COPY_DST),
        })?;
        let (timeline_start, timeline_end, timeline_source) = point_sources(context, handle)?;
        let (materialized, timeline_config, timeline_group) = if timeline_materialized(context, handle) {
            create_materialization(
                context,
                timeline_start.as_ref(),
                timeline_end.as_ref(),
                context.scene.point_frames(handle).map_or(0.0, |(_, _, alpha, _)| alpha),
                row_count,
            )?
        } else {
            (None, None, None)
        };
        write_config::<D>(
            queue,
            batch,
            &PointConfigWrite {
                buffer: &config,
                page,
                tile_extent: tiles.extent,
                tile_count: tiles.count,
                frames: context.scene.point_frames(handle),
                materialized: materialized.is_some(),
            },
        );
        let mut visual = GenericVisualState::new();
        visual.sync(
            device,
            queue,
            &GenericVisualTarget {
                scene: context.scene,
                domain: pdviewx_core::RowDomain::Points(handle),
                color: batch.style().color,
                opacity: f32::from(batch.style().color.a) / 255.0,
                row_count: count(batch),
            },
            context.visual,
        )?;
        let groups = point_groups(
            &PointGroupContext {
                device,
                cull_layout: context.cull_layout,
                render_layout: context.render_layout,
                tiles: tiles.buffer,
            },
            &PointGroupInputs {
                positions: buffer_or::<D>(materialized.as_ref(), positions.get().ok_or_else(missing_buffer)?),
                frame_end: buffer_or::<D>(timeline_end.as_ref(), positions.get().ok_or_else(missing_buffer)?),
                output: &output,
                config: &config,
                visual: visual.entries(context.visual),
            },
        );
        Ok(Self {
            handle,
            glyph: batch.glyph(),
            positions,
            timeline_start,
            timeline_end,
            timeline_source,
            materialized,
            timeline_config,
            timeline_group,
            output,
            config,
            cull_group: groups.cull,
            render_group: groups.render,
            visual,
            count: row_count,
        })
    }

    fn rebind(
        &mut self,
        device: &D,
        cull_layout: &D::BindGroupLayout,
        render_layout: &D::BindGroupLayout,
        tiles: &D::Buffer,
        visual_resources: GenericVisualResources<'_, D>,
    ) {
        let Some(positions) = self.active_start_buffer() else { return };
        let Some(frame_end) = self.active_end_buffer() else { return };
        let inputs = PointGroupInputs {
            positions,
            frame_end,
            output: &self.output,
            config: &self.config,
            visual: self.visual.entries(visual_resources),
        };
        let groups = point_groups(
            &PointGroupContext { device, cull_layout, render_layout, tiles },
            &inputs,
        );
        self.cull_group = groups.cull;
        self.render_group = groups.render;
    }
}

fn runtime_storage_bytes(header: u64, rows: u32) -> u64 {
    header
        .saturating_add(u64::from(rows).saturating_mul(4))
        .saturating_add(15)
        & !15
}

fn point_sources<D: Device>(
    context: &PointBatchSync<'_, D>,
    handle: PointBatchHandle,
) -> Result<PointSources<D>, RenderError> {
    let Some((start, end, _, _)) = context.scene.point_frames(handle) else {
        return Ok((None, None, None));
    };
    Ok((
        Some(storage_buffer(context.device, context.queue, "generic point timeline start", start.as_ref())?),
        Some(storage_buffer(context.device, context.queue, "generic point timeline end", end.as_ref())?),
        Some(frame_identity(start, end)),
    ))
}

type PointSources<D> = (
    Option<<D as Device>::Buffer>,
    Option<<D as Device>::Buffer>,
    Option<(usize, usize)>,
);
