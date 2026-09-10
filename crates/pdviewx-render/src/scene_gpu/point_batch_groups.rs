fn point_groups<D: Device>(
    context: &PointGroupContext<'_, D>,
    inputs: &PointGroupInputs<'_, D>,
) -> PointGroups<D> {
    let cull = context.device.create_bind_group(&BindGroupDesc {
        label: "group1: generic point culling",
        layout: context.cull_layout,
        entries: &[
            BindGroupEntry::Buffer { binding: 0, buffer: inputs.positions },
            BindGroupEntry::Buffer {
                binding: 1,
                buffer: inputs.output,
            },
            BindGroupEntry::Buffer {
                binding: 3,
                buffer: inputs.config,
            },
            BindGroupEntry::Buffer {
                binding: 4,
                buffer: context.tiles,
            },
            BindGroupEntry::Buffer {
                binding: 5,
                buffer: inputs.visual.instructions,
            },
            BindGroupEntry::Buffer {
                binding: 6,
                buffer: inputs.visual.parameters,
            },
            BindGroupEntry::Buffer {
                binding: 7,
                buffer: inputs.visual.properties,
            },
            BindGroupEntry::Buffer {
                binding: 8,
                buffer: inputs.visual.results,
            },
            BindGroupEntry::Buffer {
                binding: 9,
                buffer: inputs.visual.config,
            },
            BindGroupEntry::Buffer { binding: 10, buffer: inputs.frame_end },
        ],
    });
    let render = context.device.create_bind_group(&BindGroupDesc {
        label: "group2: generic point rendering",
        layout: context.render_layout,
        entries: &[
            BindGroupEntry::Buffer { binding: 0, buffer: inputs.positions },
            BindGroupEntry::Buffer {
                binding: 1,
                buffer: inputs.output,
            },
            BindGroupEntry::Buffer {
                binding: 2,
                buffer: inputs.config,
            },
            BindGroupEntry::Buffer {
                binding: 3,
                buffer: inputs.visual.properties,
            },
            BindGroupEntry::Buffer {
                binding: 4,
                buffer: inputs.visual.results,
            },
            BindGroupEntry::Buffer {
                binding: 5,
                buffer: inputs.visual.config,
            },
            BindGroupEntry::Buffer {
                binding: 6,
                buffer: inputs.visual.fragment_program,
            },
            BindGroupEntry::Buffer { binding: 7, buffer: inputs.frame_end },
        ],
    });
    PointGroups { cull, render }
}
