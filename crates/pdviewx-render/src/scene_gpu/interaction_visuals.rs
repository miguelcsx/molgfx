impl<D: Device> GpuInteractions<D> {
    fn sync_visuals(
        &mut self,
        device: &D,
        queue: &D::Queue,
        layout: &D::BindGroupLayout,
        scene: &Scene,
        resources: GenericVisualResources<'_, D>,
    ) -> Result<(), RenderError> {
        self.visuals.retain(|handle, _| {
            self.visual_plans
                .iter()
                .any(|plan| plan.source == RelationVisualSource::Scene(*handle))
        });
        for plan in &self.visual_plans {
            if let RelationVisualSource::Scene(handle) = plan.source {
                self.visuals
                    .entry(handle)
                    .or_insert_with(GenericVisualState::new)
                    .sync(
                        device,
                        queue,
                        &GenericVisualTarget {
                            scene,
                            domain: RowDomain::Relations(handle),
                            color: plan.color,
                            opacity: plan.opacity,
                            row_count: plan.count as usize,
                        },
                        resources,
                    )?;
            }
        }
        self.rebuild_cull_streams(device, queue, layout, resources, None)
    }

    fn rebuild_cull_streams(
        &mut self,
        device: &D,
        queue: &D::Queue,
        layout: &D::BindGroupLayout,
        resources: GenericVisualResources<'_, D>,
        paged_properties: Option<&D::Buffer>,
    ) -> Result<(), RenderError> {
        let (Some(base), Some(output), Some(visible), Some(args)) = (
            self.base.get(),
            self.buffer.get(),
            self.visible.get(),
            self.args.as_ref(),
        ) else {
            self.cull_streams.clear();
            return Ok(());
        };
        let shared = resources.binding_revision();
        let plan_count = if paged_properties.is_some() {
            self.visual_plans.len()
        } else {
            self.scene_visual_count
        };
        for (index, plan) in self.visual_plans.iter().copied().take(plan_count).enumerate() {
            let visual_revision = match plan.source {
                RelationVisualSource::Scene(handle) => self
                    .visuals
                    .get(&handle)
                    .map_or(0, GenericVisualState::binding_revision),
                RelationVisualSource::Paged(id) => self
                    .paged_visuals
                    .get(&id)
                    .map_or(0, PagedRelationVisualState::binding_revision),
                RelationVisualSource::Fallback => 0,
            };
            let bindings = match plan.source {
                RelationVisualSource::Paged(_) => [
                    self.paged_visual_arenas.binding_revision(),
                    0,
                    0,
                    visual_revision,
                ],
                RelationVisualSource::Scene(_) | RelationVisualSource::Fallback => {
                    [shared[0], shared[1], shared[2], visual_revision]
                }
            };
            let reusable = self
                .cull_streams
                .get(index)
                .is_some_and(|stream| stream.source == plan.source && stream.bindings == bindings);
            if reusable {
                continue;
            }
            let config = device.create_buffer(&BufferDesc {
                label: "relation cull range",
                size: std::mem::size_of::<RelationCullConfig>() as u64,
                usage: BufferUsage::UNIFORM.union(BufferUsage::COPY_DST),
            })?;
            queue.write_buffer(
                &config,
                0,
                bytemuck::bytes_of(&RelationCullConfig {
                    counts: [plan.first, plan.count, 0, 0],
                }),
            );
            let visual = self.visual_entries(plan.source, resources, paged_properties);
            let group = device.create_bind_group(&BindGroupDesc {
                label: "group1: relation visual culling",
                layout,
                entries: &[
                    BindGroupEntry::Buffer { binding: 0, buffer: base },
                    BindGroupEntry::Buffer { binding: 1, buffer: output },
                    BindGroupEntry::Buffer { binding: 2, buffer: visible },
                    BindGroupEntry::Buffer { binding: 3, buffer: args },
                    BindGroupEntry::Buffer { binding: 4, buffer: &config },
                    BindGroupEntry::Buffer { binding: 5, buffer: visual.instructions },
                    BindGroupEntry::Buffer { binding: 6, buffer: visual.parameters },
                    BindGroupEntry::Buffer { binding: 7, buffer: visual.properties },
                    BindGroupEntry::Buffer { binding: 8, buffer: visual.results },
                    BindGroupEntry::Buffer { binding: 9, buffer: visual.config },
                ],
            });
            let stream = RelationCullStream {
                source: plan.source,
                groups: workgroups_2d(u64::from(plan.count).div_ceil(64)),
                _config: config,
                group,
                bindings,
            };
            if let Some(existing) = self.cull_streams.get_mut(index) {
                *existing = stream;
            } else {
                self.cull_streams.push(stream);
            }
        }
        if paged_properties.is_some() || self.paged_synced.is_none() {
            self.cull_streams.truncate(plan_count);
        }
        Ok(())
    }

    fn visual_entries<'a>(
        &'a self,
        source: RelationVisualSource,
        resources: GenericVisualResources<'a, D>,
        paged_properties: Option<&'a D::Buffer>,
    ) -> VisualCullEntries<'a, D> {
        match source {
            RelationVisualSource::Scene(handle) => self
                .visuals
                .get(&handle)
                .map_or(resources.fallback, |state| state.entries(resources)),
            RelationVisualSource::Paged(id) => match paged_properties
                .and_then(|properties| {
                    self.paged_visuals
                        .get(&id)
                        .and_then(|state| state.entries(properties, &self.paged_visual_arenas))
                }) {
                    Some(entries) => entries,
                    None => resources.fallback,
                },
            RelationVisualSource::Fallback => resources.fallback,
        }
    }
}
