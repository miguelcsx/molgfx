fn instance_groups<D: Device>(
    context: &InstanceGroupContext<'_, D>,
    inputs: &InstanceGroupInputs<'_, D>,
) -> InstanceGroups<D> {
    let cull = context.device.create_bind_group(&BindGroupDesc {
        label: "group1: generic instance culling",
        layout: context.cull_layout,
        entries: &[
            buffer_entry(0, inputs.transforms),
            buffer_entry(1, inputs.args),
            buffer_entry(3, inputs.config),
            buffer_entry(4, inputs.visual.instructions),
            buffer_entry(5, inputs.visual.parameters),
            buffer_entry(6, inputs.visual.properties),
            buffer_entry(7, inputs.visual.results),
            buffer_entry(8, inputs.visual.config),
            buffer_entry(9, inputs.frame_start),
            buffer_entry(10, inputs.frame_end),
        ],
    });
    let render = context.device.create_bind_group(&BindGroupDesc {
        label: "group2: generic analytic instances",
        layout: context.render_layout,
        entries: &[
            buffer_entry(5, &inputs.template.spheres),
            buffer_entry(6, &inputs.template.capsules),
            buffer_entry(7, inputs.transforms),
            buffer_entry(8, inputs.args),
            buffer_entry(9, inputs.config),
            buffer_entry(10, inputs.visual.properties),
            buffer_entry(11, inputs.visual.results),
            buffer_entry(12, inputs.visual.config),
            buffer_entry(13, inputs.visual.fragment_program),
            buffer_entry(14, inputs.frame_start),
            buffer_entry(15, inputs.frame_end),
        ],
    });
    InstanceGroups { cull, render }
}
