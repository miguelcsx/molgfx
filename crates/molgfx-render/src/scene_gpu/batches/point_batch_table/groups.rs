use super::{
    BindGroupDesc, Device, PointGroupContext, PointGroupInputs, PointGroups, buffer_entry,
};

pub(super) fn point_groups<D: Device>(
    context: &PointGroupContext<'_, D>,
    inputs: &PointGroupInputs<'_, D>,
) -> PointGroups<D> {
    let cull = context.device.create_bind_group(&BindGroupDesc {
        label: "group1: generic point culling",
        layout: context.cull_layout,
        entries: &[
            buffer_entry(0, inputs.positions),
            buffer_entry(1, inputs.output),
            buffer_entry(3, inputs.config),
            buffer_entry(4, context.tiles),
            buffer_entry(5, inputs.visual.instructions),
            buffer_entry(6, inputs.visual.parameters),
            buffer_entry(7, inputs.visual.properties),
            buffer_entry(8, inputs.visual.results),
            buffer_entry(9, inputs.visual.config),
            buffer_entry(10, inputs.frame_end),
        ],
    });
    let render = context.device.create_bind_group(&BindGroupDesc {
        label: "group2: generic point rendering",
        layout: context.render_layout,
        entries: &[
            buffer_entry(0, inputs.positions),
            buffer_entry(1, inputs.output),
            buffer_entry(2, inputs.config),
            buffer_entry(3, inputs.visual.properties),
            buffer_entry(4, inputs.visual.results),
            buffer_entry(5, inputs.visual.config),
            buffer_entry(6, inputs.visual.fragment_program),
            buffer_entry(7, inputs.frame_end),
        ],
    });
    PointGroups { cull, render }
}
