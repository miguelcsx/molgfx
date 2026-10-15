// Bind-group lowering for homogeneous relation-anchor streams.

impl<D: Device> GpuInteractions<D> {
    fn bind_streams(
        &mut self,
        device: &D,
        layout: &D::BindGroupLayout,
        sources: RelationSources<'_, D>,
        plans: &[StreamPlan],
    ) -> Result<(), RenderError> {
        self.streams.clear();
        let Some(resolver_buffer) = self.resolvers.get() else {
            return Ok(());
        };
        let Some(output_buffer) = self.buffer.get() else {
            return Ok(());
        };
        for plan in plans {
            let [start_a, start_b, start_config] =
                self.source_entries(plan.key.start_domain, [1, 6, 8], sources)?;
            let [end_a, end_b, end_config] =
                self.source_entries(plan.key.end_domain, [2, 7, 9], sources)?;
            let entries = [
                BindGroupEntry::BufferRange {
                    binding: 0,
                    buffer: resolver_buffer,
                    offset: u64::from(plan.first)
                        * std::mem::size_of::<RelationResolverGpu>() as u64,
                    size: u64::from(plan.count) * std::mem::size_of::<RelationResolverGpu>() as u64,
                },
                start_a,
                end_a,
                BindGroupEntry::Buffer {
                    binding: 3,
                    buffer: output_buffer,
                },
                self.model_entry(plan.key.start_domain, 4, sources)?,
                self.model_entry(plan.key.end_domain, 5, sources)?,
                start_b,
                end_b,
                start_config,
                end_config,
            ];
            self.streams.push(RelationStream {
                pipeline: pipeline_index(plan.key),
                tracks_coordinates: matches!(
                    (plan.key.start, plan.key.end),
                    (
                        super::relation_anchor::AnchorKernel::Point
                            | super::relation_anchor::AnchorKernel::Atom
                            | super::relation_anchor::AnchorKernel::Rigid,
                        _,
                    ) | (
                        _,
                        super::relation_anchor::AnchorKernel::Point
                            | super::relation_anchor::AnchorKernel::Atom
                            | super::relation_anchor::AnchorKernel::Rigid,
                    )
                ),
                groups: workgroups_2d(u64::from(plan.count).div_ceil(64)),
                group: device.create_bind_group(&BindGroupDesc {
                    label: "group1: dynamic relation resolver",
                    layout,
                    entries: &entries,
                }),
                _start_model: None,
                _end_model: None,
                start_timeline: None,
                end_timeline: None,
                start_config: None,
                end_config: None,
            });
        }
        Ok(())
    }

    fn source_entries<'a>(
        &'a self,
        domain: Option<RowDomain>,
        bindings: [u32; 3],
        sources: RelationSources<'a, D>,
    ) -> Result<[BindGroupEntry<'a, D>; 3], RenderError> {
        if let Some(RowDomain::Instances(handle)) = domain {
            return sources
                .instances
                .relation_source_entries(handle, bindings)
                .ok_or_else(|| missing_domain(domain));
        }
        if let Some(RowDomain::Points(handle)) = domain {
            return sources
                .points
                .relation_source_entries(handle, bindings)
                .ok_or_else(|| missing_domain(domain));
        }
        let (start, end) = self.static_source_entries(domain, [bindings[0], bindings[1]], sources)?;
        let timeline = self
            .timeline_fallback
            .as_ref()
            .ok_or_else(|| missing_domain(domain))?;
        Ok([
            start,
            end,
            BindGroupEntry::Buffer {
                binding: bindings[2],
                buffer: timeline,
            },
        ])
    }

    fn static_source_entries<'a>(
        &'a self,
        domain: Option<RowDomain>,
        bindings: [u32; 2],
        sources: RelationSources<'a, D>,
    ) -> Result<(BindGroupEntry<'a, D>, BindGroupEntry<'a, D>), RenderError> {
        match domain {
            None => {
                let source = self.source_fallback.as_ref().ok_or_else(|| missing_domain(domain))?;
                Ok((
                    BindGroupEntry::Buffer { binding: bindings[0], buffer: source },
                    BindGroupEntry::Buffer { binding: bindings[1], buffer: source },
                ))
            }
            Some(RowDomain::Atoms(handle)) => sources
                .structures
                .iter()
                .find(|source| source.handle == handle)
                .map(|source| (
                    source.coords_entry(sources.asset_arena, bindings[0]),
                    source.coords_entry(sources.asset_arena, bindings[1]),
                ))
                .ok_or_else(|| missing_domain(domain)),
            Some(RowDomain::Points(_) | RowDomain::Instances(_)) => Err(missing_domain(domain)),
            Some(domain @ (RowDomain::TemplateParts(_) | RowDomain::Relations(_))) => {
                Err(RenderError::RelationSourceMissing { domain })
            }
        }
    }

    fn model_entry<'a>(
        &'a self,
        domain: Option<RowDomain>,
        binding: u32,
        sources: RelationSources<'a, D>,
    ) -> Result<BindGroupEntry<'a, D>, RenderError> {
        if let Some(RowDomain::Atoms(handle)) = domain {
            return sources.structures.iter().find(|source| source.handle == handle)
                .and_then(|source| source.model_entry(binding))
                .ok_or(RenderError::RelationSourceMissing { domain: RowDomain::Atoms(handle) });
        }
        self.model_fallback.as_ref()
            .map(|buffer| BindGroupEntry::Buffer { binding, buffer })
            .ok_or_else(|| missing_domain(domain))
    }

    fn ensure_fallbacks(&mut self, device: &D, queue: &D::Queue) -> Result<(), RenderError> {
        if self.source_fallback.is_none() {
            self.source_fallback = Some(device.create_buffer(&BufferDesc {
                label: "dynamic relation unused source", size: 256,
                usage: BufferUsage::STORAGE.union(BufferUsage::COPY_DST),
            })?);
        }
        if self.model_fallback.is_none() {
            let buffer = device.create_buffer(&BufferDesc {
                label: "dynamic relation identity model",
                size: std::mem::size_of::<ModelUniforms>() as u64,
                usage: BufferUsage::UNIFORM.union(BufferUsage::COPY_DST),
            })?;
            queue.write_buffer(&buffer, 0, bytemuck::bytes_of(&ModelUniforms::new(
                Mat4::IDENTITY, Mat4::IDENTITY, [0; 9],
            )));
            self.model_fallback = Some(buffer);
        }
        if self.timeline_fallback.is_none() {
            let buffer = device.create_buffer(&BufferDesc {
                label: "dynamic relation static timeline", size: 48,
                usage: BufferUsage::UNIFORM.union(BufferUsage::COPY_DST),
            })?;
            queue.write_buffer(&buffer, 0, &[0; 48]);
            self.timeline_fallback = Some(buffer);
        }
        Ok(())
    }
}
