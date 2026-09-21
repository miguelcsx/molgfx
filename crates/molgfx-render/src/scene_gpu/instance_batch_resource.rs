// Persistent resources for one generic analytic instance batch.

impl<D: Device> GpuInstanceBatch<D> {
    fn new(
        context: &InstanceBatchSync<'_, D>,
        handle: InstanceBatchHandle,
        batch: &InstanceBatch,
        page: u32,
        template: Arc<GpuAnalyticTemplate<D>>,
    ) -> Result<Self, RenderError> {
        let device = context.device;
        let queue = context.queue;
        let count = batch.source_rows().len();
        let sphere_count = checked_count(batch.template().spheres().len())?;
        let capsule_count = checked_count(batch.template().capsules().len())?;
        let transforms = storage_buffer(
            device,
            queue,
            "generic rigid instance transforms",
            batch.transforms().as_ref(),
        )?;
        let mut timeline_start = None;
        let mut timeline_end = None;
        let mut timeline_source = None;
        if let Some((start, end, _, _)) = context.scene.instance_frames(handle) {
            timeline_start = Some(storage_buffer(
                device, queue, "generic instance timeline start", start.as_ref(),
            )?);
            timeline_end = Some(storage_buffer(
                device, queue, "generic instance timeline end", end.as_ref(),
            )?);
            timeline_source = Some(frame_identity(start, end));
        }
        let (materialized, timeline_config, timeline_group) = if timeline_materialized(context, handle) {
            create_materialization(
                context,
                timeline_start.as_ref(),
                timeline_end.as_ref(),
                context
                    .scene
                    .instance_frames(handle)
                    .map_or(0.0, |(_, _, alpha, _)| alpha),
                count,
            )?
        } else {
            (None, None, None)
        };
        let args = instance_args::<D>(device, queue, count)?;
        let config = device.create_buffer(&BufferDesc {
            label: "generic instance configuration",
            size: std::mem::size_of::<GenericInstanceConfig>() as u64,
            usage: BufferUsage::UNIFORM.union(BufferUsage::COPY_DST),
        })?;
        write_config::<D>(
            queue,
            &config,
            batch,
            page,
            context.scene.instance_frames(handle),
            materialized.is_some(),
        )?;
        let mut visual = GenericVisualState::new();
        visual.sync(
            device,
            queue,
            &GenericVisualTarget {
                scene: context.scene,
                domain: molgfx_core::RowDomain::Instances(handle),
                color: batch.style().color,
                opacity: f32::from(batch.style().color.a) / 255.0,
                row_count: batch.source_rows().len() as usize,
            },
            context.visual,
        )?;
        let groups = instance_groups(
            &InstanceGroupContext {
                device,
                cull_layout: context.cull_layout,
                render_layout: context.render_layout,
            },
            &InstanceGroupInputs {
                template: &template,
                transforms: buffer_or::<D>(materialized.as_ref(), &transforms),
                frame_start: buffer_or::<D>(timeline_start.as_ref(), &transforms),
                frame_end: buffer_or::<D>(timeline_end.as_ref(), &transforms),
                args: &args,
                config: &config,
                visual: visual.entries(context.visual),
            },
        );
        Ok(Self {
            handle,
            template,
            transforms,
            timeline_start,
            timeline_end,
            timeline_source,
            materialized,
            timeline_config,
            timeline_group,
            args,
            config,
            cull_group: groups.cull,
            render_group: groups.render,
            visual,
            count,
            sphere_count,
            capsule_count,
        })
    }

    fn rebind(
        &mut self,
        device: &D,
        cull_layout: &D::BindGroupLayout,
        render_layout: &D::BindGroupLayout,
        visual_resources: GenericVisualResources<'_, D>,
    ) {
        let groups = instance_groups(
            &InstanceGroupContext { device, cull_layout, render_layout },
            &InstanceGroupInputs {
                template: &self.template,
                transforms: buffer_or::<D>(self.materialized.as_ref(), &self.transforms),
                frame_start: buffer_or::<D>(self.timeline_start.as_ref(), &self.transforms),
                frame_end: buffer_or::<D>(self.timeline_end.as_ref(), &self.transforms),
                args: &self.args,
                config: &self.config,
                visual: self.visual.entries(visual_resources),
            },
        );
        self.cull_group = groups.cull;
        self.render_group = groups.render;
    }

    fn sync_timeline(
        &mut self,
        context: &InstanceBatchSync<'_, D>,
        _batch: &InstanceBatch,
    ) -> Result<bool, RenderError> {
        let Some((start, end, alpha, _)) = context.scene.instance_frames(self.handle) else {
            let changed = self.timeline_start.take().is_some() || self.timeline_end.take().is_some();
            self.timeline_source = None;
            self.materialized = None;
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
                "generic instance timeline start",
                start.as_ref(),
            )?);
            self.timeline_end = Some(storage_buffer(
                context.device,
                context.queue,
                "generic instance timeline end",
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
            context.queue.write_buffer(
                config,
                0,
                bytemuck::bytes_of(&InstanceTimelineConfig {
                    values: [alpha, 0.0, 0.0, 0.0],
                    counts: [self.count, 0, 0, 0],
                }),
            );
        } else {
            self.materialized = None;
            self.timeline_config = None;
            self.timeline_group = None;
        }
        Ok(source_changed || mode_changed)
    }
}

fn buffer_or<'a, D: Device>(candidate: Option<&'a D::Buffer>, fallback: &'a D::Buffer) -> &'a D::Buffer {
    match candidate {
        Some(buffer) => buffer,
        None => fallback,
    }
}

fn create_materialization<D: Device>(
    context: &InstanceBatchSync<'_, D>,
    start: Option<&D::Buffer>,
    end: Option<&D::Buffer>,
    alpha: f32,
    count: u32,
) -> Result<MaterializationResources<D>, RenderError> {
    let (Some(start), Some(end)) = (start, end) else {
        return Ok((None, None, None));
    };
    let output = context.device.create_buffer(&BufferDesc {
        label: "materialized generic instance timeline",
        size: u64::from(count).saturating_mul(32).max(16),
        usage: BufferUsage::STORAGE,
    })?;
    let config = context.device.create_buffer(&BufferDesc {
        label: "generic instance timeline configuration",
        size: std::mem::size_of::<InstanceTimelineConfig>() as u64,
        usage: BufferUsage::UNIFORM.union(BufferUsage::COPY_DST),
    })?;
    context.queue.write_buffer(
        &config,
        0,
        bytemuck::bytes_of(&InstanceTimelineConfig {
            values: [alpha, 0.0, 0.0, 0.0],
            counts: [count, 0, 0, 0],
        }),
    );
    let group = context.device.create_bind_group(&BindGroupDesc {
        label: "group0: generic instance timeline",
        layout: context.timeline_layout,
        entries: &[
            BindGroupEntry::Buffer {
                binding: 0,
                buffer: start,
            },
            BindGroupEntry::Buffer {
                binding: 1,
                buffer: end,
            },
            BindGroupEntry::Buffer {
                binding: 2,
                buffer: &output,
            },
            BindGroupEntry::Buffer {
                binding: 3,
                buffer: &config,
            },
        ],
    });
    Ok((Some(output), Some(config), Some(group)))
}

fn instance_args<D: Device>(device: &D, queue: &D::Queue, count: u32) -> Result<D::Buffer, RenderError> {
    let args = device.create_buffer(&BufferDesc {
        label: "generic instance cull output",
        size: runtime_storage_bytes(
            (std::mem::size_of::<DrawIndirectArgs>() * 2) as u64,
            count,
        ),
        usage: BufferUsage::INDIRECT.union(BufferUsage::STORAGE).union(BufferUsage::COPY_DST),
    })?;
    queue.write_buffer(
        &args,
        0,
        bytemuck::cast_slice(&[
            DrawIndirectArgs { vertex_count: 6, instance_count: 0, first_vertex: 0, first_instance: 0 },
            DrawIndirectArgs { vertex_count: 6, instance_count: 0, first_vertex: 0, first_instance: 0 },
        ]),
    );
    Ok(args)
}

fn runtime_storage_bytes(header: u64, rows: u32) -> u64 {
    header
        .saturating_add(u64::from(rows).saturating_mul(4))
        .saturating_add(15)
        & !15
}
