// Resident chunk-backed rigid instances using the shared analytic pipelines.

#[derive(Debug)]
struct GpuPagedInstanceBatch<D: Device> {
    source_revision: u64,
    plan: crate::engine::chunk_draw_plan::ResidentInstanceChunkPlacement,
    template: Arc<GpuAnalyticTemplate<D>>,
    args: D::Buffer,
    config: D::Buffer,
    materialized: Option<D::Buffer>,
    timeline_config: Option<D::Buffer>,
    timeline_group: Option<D::BindGroup>,
    cull_group: D::BindGroup,
    render_group: D::BindGroup,
    visual: GenericVisualState<D>,
    count: u32,
    sphere_count: u32,
    capsule_count: u32,
}

impl<D: Device> GpuPagedInstanceBatch<D> {
    #[allow(clippy::too_many_arguments)]
    fn new(
        device: &D,
        queue: &D::Queue,
        cull_layout: &D::BindGroupLayout,
        render_layout: &D::BindGroupLayout,
        timeline_layout: &D::BindGroupLayout,
        source: &D::Buffer,
        source_revision: u64,
        page: u32,
        plan: &crate::engine::chunk_draw_plan::ResidentInstanceChunkPlacement,
        template: Arc<GpuAnalyticTemplate<D>>,
        resources: GenericVisualResources<'_, D>,
        materialize: bool,
    ) -> Result<Self, RenderError> {
        let count = plan.span.row_count();
        let sphere_count = checked_count(plan.template.spheres().len())?;
        let capsule_count = checked_count(plan.template.capsules().len())?;
        let args = instance_args::<D>(device, queue, count)?;
        let config = device.create_buffer(&BufferDesc {
            label: "paged instance configuration",
            size: std::mem::size_of::<GenericInstanceConfig>() as u64,
            usage: BufferUsage::UNIFORM.union(BufferUsage::COPY_DST),
        })?;
        let (materialized, timeline_config, timeline_group) =
            create_paged_materialization(device, queue, timeline_layout, source, plan, materialize)?;
        write_paged_instance_config::<D>(queue, &config, page, plan, materialized.is_some())?;
        let mut visual = GenericVisualState::new();
        visual.sync_uniform(
            device,
            queue,
            plan.color,
            f32::from(plan.color.a) / 255.0,
            count as usize,
            resources,
        )?;
        let groups = instance_groups(
            &InstanceGroupContext {
                device,
                cull_layout,
                render_layout,
            },
            &InstanceGroupInputs {
                template: &template,
                transforms: match &materialized {
                    Some(transforms) => transforms,
                    None => source,
                },
                frame_start: source,
                frame_end: source,
                args: &args,
                config: &config,
                visual: visual.entries(resources),
            },
        );
        Ok(Self {
            source_revision,
            plan: plan.clone(),
            template,
            args,
            config,
            materialized,
            timeline_config,
            timeline_group,
            cull_group: groups.cull,
            render_group: groups.render,
            visual,
            count,
            sphere_count,
            capsule_count,
        })
    }

    fn sync_plan(
        &mut self,
        queue: &D::Queue,
        page: u32,
        plan: &crate::engine::chunk_draw_plan::ResidentInstanceChunkPlacement,
    ) -> Result<bool, RenderError> {
        if self.plan == *plan {
            return Ok(false);
        }
        if self.materialized.is_none() || !same_materialized_config(&self.plan, plan) {
            write_paged_instance_config::<D>(
                queue,
                &self.config,
                page,
                plan,
                self.materialized.is_some(),
            )?;
        }
        if let (Some(config), Some(timeline)) = (&self.timeline_config, plan.timeline) {
            queue.write_buffer(config, 0, bytemuck::bytes_of(&timeline_config(timeline, plan.span.row_count())?));
        }
        self.plan = plan.clone();
        Ok(true)
    }
}

fn same_materialized_config(
    left: &crate::engine::chunk_draw_plan::ResidentInstanceChunkPlacement,
    right: &crate::engine::chunk_draw_plan::ResidentInstanceChunkPlacement,
) -> bool {
    left.id == right.id
        && left.ticket == right.ticket
        && left.byte_offset == right.byte_offset
        && left.span == right.span
        && Arc::ptr_eq(&left.template, &right.template)
        && left.color == right.color
        && left.timeline.is_some() == right.timeline.is_some()
}

impl<D: Device> GpuInstanceBatches<D> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn sync_paged(
        &mut self,
        device: &D,
        queue: &D::Queue,
        cull_layout: &D::BindGroupLayout,
        render_layout: &D::BindGroupLayout,
        timeline_layout: &D::BindGroupLayout,
        source: &D::Buffer,
        source_revision: u64,
        picking: &PickPages,
        plans: &[crate::engine::chunk_draw_plan::ResidentInstanceChunkPlacement],
        resources: GenericVisualResources<'_, D>,
        derived_cache: &mut DerivedCache,
        frame: u64,
    ) -> Result<bool, RenderError> {
        self.plan_paged_materializations(plans, derived_cache, frame);
        if self.paged.len() == plans.len()
            && self
                .paged
                .iter()
                .zip(plans)
                .zip(&self.paged_materialization)
                .all(|((resident, plan), materialized)| {
                resident.source_revision == source_revision
                    && Arc::ptr_eq(&resident.plan.template, &plan.template)
                    && resident.plan.span.row_count() == plan.span.row_count()
                    && resident.materialized.is_some() == *materialized
                })
        {
            let mut changed = false;
            for (resident, plan) in self.paged.iter_mut().zip(plans) {
                let page = picking
                    .page_for_chunk(
                        plan.ticket.key.dataset,
                        plan.ticket.key.chunk,
                        EntityKind::TemplatePart,
                    )
                    .ok_or(RenderError::PickingOwnerMissing)?;
                changed |= resident.sync_plan(queue, page, plan)?;
            }
            return Ok(changed);
        }
        let cached: Vec<_> = self
            .batches
            .iter()
            .map(|batch| Arc::clone(&batch.template))
            .chain(self.paged.iter().map(|batch| Arc::clone(&batch.template)))
            .collect();
        let mut rebuilt = Vec::with_capacity(plans.len());
        for (plan, materialize) in plans.iter().zip(&self.paged_materialization) {
            let page = picking
                .page_for_chunk(
                    plan.ticket.key.dataset,
                    plan.ticket.key.chunk,
                    EntityKind::TemplatePart,
                )
                .ok_or(RenderError::PickingOwnerMissing)?;
            let template = cached
                .iter()
                .find(|entry| Arc::ptr_eq(&entry.source, &plan.template))
                .cloned()
                .map_or_else(
                    || GpuAnalyticTemplate::new(device, queue, &plan.template),
                    Ok,
                )?;
            rebuilt.push(GpuPagedInstanceBatch::new(
                device,
                queue,
                cull_layout,
                render_layout,
                timeline_layout,
                source,
                source_revision,
                page,
                plan,
                template,
                resources,
                *materialize,
            )?);
        }
        self.paged = rebuilt;
        self.paged_binding_revision = self.paged_binding_revision.wrapping_add(1);
        Ok(true)
    }

    fn plan_paged_materializations(
        &mut self,
        plans: &[crate::engine::chunk_draw_plan::ResidentInstanceChunkPlacement],
        derived_cache: &mut DerivedCache,
        frame: u64,
    ) {
        for resident in &self.paged {
            if !plans.iter().any(|plan| plan.id == resident.plan.id && plan.timeline.is_some()) {
                derived_cache.release(paged_timeline_cache_key(resident.plan.id));
            }
        }
        for plan in plans {
            let key = paged_timeline_cache_key(plan.id);
            if plan.timeline.is_none() {
                derived_cache.release(key);
                continue;
            }
            let _plan = derived_cache.plan(
                key,
                DerivedFootprint {
                    cpu_bytes: 0,
                    gpu_bytes: u64::from(plan.span.row_count()).saturating_mul(32),
                },
                2,
                frame,
            );
        }
        let _evicted = derived_cache.take_evictions().count();
        self.paged_materialization.clear();
        self.paged_materialization.extend(plans.iter().map(|plan| {
            plan.timeline.is_some()
                && derived_cache.contains(paged_timeline_cache_key(plan.id))
        }));
    }

    pub(super) const fn paged_binding_revision(&self) -> u64 {
        self.paged_binding_revision
    }

    pub(super) fn paged_materialized_source(
        &self,
        start_byte_offset: u64,
        end_byte_offset: u64,
    ) -> Option<&D::Buffer> {
        self.paged.iter().find_map(|batch| {
            let timeline = batch.plan.timeline?;
            (timeline.start_byte_offset == start_byte_offset
                && timeline.end_byte_offset == end_byte_offset)
                .then_some(batch.materialized.as_ref())
                .flatten()
        })
    }
}

fn write_paged_instance_config<D: Device>(
    queue: &D::Queue,
    config: &D::Buffer,
    page: u32,
    plan: &crate::engine::chunk_draw_plan::ResidentInstanceChunkPlacement,
    materialized: bool,
) -> Result<(), RenderError> {
    let base = if materialized {
        0
    } else {
        instance_word_offset(plan.byte_offset)?
    };
    let (timeline_start, timeline_end, interpolation, timeline_enabled) =
        if let Some(timeline) = plan.timeline.filter(|_| !materialized) {
            (
                instance_word_offset(timeline.start_byte_offset)?,
                instance_word_offset(timeline.end_byte_offset)?,
                timeline.interpolation,
                1.0,
            )
        } else {
            (0, 0, 0.0, 0.0)
        };
    let sphere_count = checked_count(plan.template.spheres().len())?;
    let capsule_count = checked_count(plan.template.capsules().len())?;
    let part_count = sphere_count.checked_add(capsule_count).ok_or_else(limit)?;
    queue.write_buffer(
        config,
        0,
        bytemuck::bytes_of(&GenericInstanceConfig {
            counts: [
                plan.span.row_count(),
                sphere_count,
                capsule_count,
                part_count,
            ],
            picking_style: [page, pack_color(plan.color), base, timeline_start],
            bounds: [
                plan.template.bound_radius(),
                interpolation,
                timeline_enabled,
                f32::from_bits(timeline_end),
            ],
        }),
    );
    Ok(())
}

fn create_paged_materialization<D: Device>(
    device: &D,
    queue: &D::Queue,
    layout: &D::BindGroupLayout,
    source: &D::Buffer,
    plan: &crate::engine::chunk_draw_plan::ResidentInstanceChunkPlacement,
    materialize: bool,
) -> Result<MaterializationResources<D>, RenderError> {
    let Some(timeline) = plan.timeline.filter(|_| materialize) else {
        return Ok((None, None, None));
    };
    let output = device.create_buffer(&BufferDesc {
        label: "materialized paged instance timeline",
        size: u64::from(plan.span.row_count()).saturating_mul(32).max(16),
        usage: BufferUsage::STORAGE,
    })?;
    let config = device.create_buffer(&BufferDesc {
        label: "paged instance timeline configuration",
        size: std::mem::size_of::<InstanceTimelineConfig>() as u64,
        usage: BufferUsage::UNIFORM.union(BufferUsage::COPY_DST),
    })?;
    queue.write_buffer(
        &config,
        0,
        bytemuck::bytes_of(&timeline_config(timeline, plan.span.row_count())?),
    );
    let group = device.create_bind_group(&BindGroupDesc {
        label: "group0: paged instance timeline",
        layout,
        entries: &[
            BindGroupEntry::Buffer {
                binding: 0,
                buffer: source,
            },
            BindGroupEntry::Buffer {
                binding: 1,
                buffer: source,
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

fn timeline_config(
    timeline: crate::engine::chunk_draw_plan::ResidentInstanceChunkWindow,
    count: u32,
) -> Result<InstanceTimelineConfig, RenderError> {
    Ok(InstanceTimelineConfig {
        values: [timeline.interpolation, 0.0, 0.0, 0.0],
        counts: [
            count,
            instance_word_offset(timeline.start_byte_offset)?,
            instance_word_offset(timeline.end_byte_offset)?,
            0,
        ],
    })
}

const fn paged_timeline_cache_key(
    id: pdviewx_core::ChunkOccurrenceId,
) -> crate::engine::DerivedCacheKey {
    crate::engine::DerivedCacheKey::PagedInstance(id)
}

fn instance_word_offset(byte_offset: u64) -> Result<u32, RenderError> {
    if !byte_offset.is_multiple_of(32) {
        return Err(limit());
    }
    u32::try_from(byte_offset / 32).map_err(|_| limit())
}

fn paged_instance_dispatch<D: Device>(
    batch: &GpuPagedInstanceBatch<D>,
) -> GenericInstanceDispatch<'_, D> {
    GenericInstanceDispatch {
        group: &batch.cull_group,
        groups: workgroups_2d(u64::from(batch.count).div_ceil(64)),
        cull_visual: batch.visual.has_cull_results(),
        shading_visual: batch.visual.has_shading_results(),
    }
}

fn paged_instance_draws<D: Device>(
    batch: &GpuPagedInstanceBatch<D>,
) -> impl Iterator<Item = GenericInstanceDraw<'_, D>> {
    [
        (batch.sphere_count != 0).then_some(GenericInstanceDraw {
            group: &batch.render_group,
            args: &batch.args,
            args_offset: 0,
            shape: GENERIC_INSTANCE_SPHERE,
            shading: batch.visual.shading(),
        }),
        (batch.capsule_count != 0).then_some(GenericInstanceDraw {
            group: &batch.render_group,
            args: &batch.args,
            args_offset: std::mem::size_of::<DrawIndirectArgs>() as u64,
            shape: GENERIC_INSTANCE_CAPSULE,
            shading: batch.visual.shading(),
        }),
    ]
    .into_iter()
    .flatten()
}
