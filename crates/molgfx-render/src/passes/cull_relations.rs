// Specialized relation visibility pipelines and recording.

struct RelationPipelines<D: Device> {
    reset: D::Pipeline,
    cull: D::Pipeline,
}

impl<D: Device> RelationPipelines<D> {
    fn new(
        device: &D,
        frame: &D::BindGroupLayout,
        relations: &D::BindGroupLayout,
    ) -> Result<Self, RenderError> {
        let shader = shader(
            device,
            "generic relation culling",
            molgfx_shaders::RELATION_CULL,
        )?;
        let layouts = &[Some(frame), Some(relations)];
        Ok(Self {
            reset: pipeline(
                device,
                &shader,
                layouts,
                "reset generic relations",
                "reset_relations",
            )?,
            cull: pipeline(
                device,
                &shader,
                layouts,
                "cull generic relations",
                "cull_relations",
            )?,
        })
    }
}

fn record_relations<D: Device, P: ComputePassEncoder<D>>(
    pass: &mut P,
    scene: &crate::scene_gpu::GpuScene<D>,
    cull: &CullPass<D>,
) {
    let mut dispatches = scene.relation_cull_dispatches();
    let Some(first) = dispatches.next() else {
        return;
    };
    pass.set_bind_group(0, &scene.group0, &[]);
    pass.set_bind_group(1, first.group, &[]);
    pass.set_pipeline(&cull.relation_reset);
    pass.dispatch(1, 1, 1);
    pass.set_pipeline(&cull.relation_cull);
    pass.dispatch(first.groups[0], first.groups[1], 1);
    for dispatch in dispatches {
        pass.set_bind_group(1, dispatch.group, &[]);
        pass.dispatch(dispatch.groups[0], dispatch.groups[1], 1);
    }
}
