// Shared compute shader and pipeline construction.

fn shader<D: Device>(
    device: &D,
    label: &'static str,
    wgsl: &'static str,
) -> Result<D::ShaderModule, RenderError> {
    device
        .create_shader_module(&ShaderModuleDesc { label, wgsl })
        .map_err(Into::into)
}

fn pipeline<D: Device>(
    device: &D,
    shader: &D::ShaderModule,
    layouts: &[Option<&D::BindGroupLayout>],
    label: &'static str,
    entry: &'static str,
) -> Result<D::Pipeline, RenderError> {
    device
        .create_compute_pipeline(&ComputePipelineDesc {
            label,
            layouts,
            shader,
            entry,
        })
        .map_err(Into::into)
}
