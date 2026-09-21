fn instance_groups<D: Device>(
    context: &InstanceGroupContext<'_, D>,
    inputs: &InstanceGroupInputs<'_, D>,
) -> InstanceGroups<D> {
    let cull = context.device.create_bind_group(&BindGroupDesc {
        label: "group1: generic instance culling",
        layout: context.cull_layout,
        entries: &[
            BindGroupEntry::Buffer {
                binding: 0,
                buffer: inputs.transforms,
            },
            BindGroupEntry::Buffer {
                binding: 1,
                buffer: inputs.args,
            },
            BindGroupEntry::Buffer {
                binding: 3,
                buffer: inputs.config,
            },
            BindGroupEntry::Buffer {
                binding: 4,
                buffer: inputs.visual.instructions,
            },
            BindGroupEntry::Buffer {
                binding: 5,
                buffer: inputs.visual.parameters,
            },
            BindGroupEntry::Buffer {
                binding: 6,
                buffer: inputs.visual.properties,
            },
            BindGroupEntry::Buffer {
                binding: 7,
                buffer: inputs.visual.results,
            },
            BindGroupEntry::Buffer {
                binding: 8,
                buffer: inputs.visual.config,
            },
            BindGroupEntry::Buffer {
                binding: 9,
                buffer: inputs.frame_start,
            },
            BindGroupEntry::Buffer {
                binding: 10,
                buffer: inputs.frame_end,
            },
        ],
    });
    let render = context.device.create_bind_group(&BindGroupDesc {
        label: "group2: generic analytic instances",
        layout: context.render_layout,
        entries: &[
            BindGroupEntry::Buffer {
                binding: 5,
                buffer: &inputs.template.spheres,
            },
            BindGroupEntry::Buffer {
                binding: 6,
                buffer: &inputs.template.capsules,
            },
            BindGroupEntry::Buffer {
                binding: 7,
                buffer: inputs.transforms,
            },
            BindGroupEntry::Buffer {
                binding: 8,
                buffer: inputs.args,
            },
            BindGroupEntry::Buffer {
                binding: 9,
                buffer: inputs.config,
            },
            BindGroupEntry::Buffer {
                binding: 10,
                buffer: inputs.visual.properties,
            },
            BindGroupEntry::Buffer {
                binding: 11,
                buffer: inputs.visual.results,
            },
            BindGroupEntry::Buffer {
                binding: 12,
                buffer: inputs.visual.config,
            },
            BindGroupEntry::Buffer {
                binding: 13,
                buffer: inputs.visual.fragment_program,
            },
            BindGroupEntry::Buffer {
                binding: 14,
                buffer: inputs.frame_start,
            },
            BindGroupEntry::Buffer {
                binding: 15,
                buffer: inputs.frame_end,
            },
        ],
    });
    InstanceGroups { cull, render }
}
