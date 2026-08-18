//! Reprojected HDR history resolve for analytic molecular silhouettes.
//!
//! Work is `O(pixels)` with a fixed 3x3 neighbourhood. Two persistent pooled
//! history textures alternate read/write roles without copies or allocations.

use crate::error::RenderError;
use crate::graph::PassContext;
use crate::passes::{FrameBindings, HISTORY_A_RESOURCE, HISTORY_B_RESOURCE};
use pdviewx_gpu::{
    BindGroupLayoutDesc, BindGroupLayoutEntry, BindingType, ColorAttachment, ColorTarget,
    CommandEncoder as _, Device, LoadOp, PrimitiveTopology, RenderPassDesc, RenderPassEncoder as _,
    RenderPipelineDesc, SamplerDesc, ShaderModuleDesc, ShaderStages, TextureFormat,
};

#[derive(Debug)]
pub struct TemporalPass<D: Device> {
    pipeline: D::Pipeline,
    pub(crate) layout: D::BindGroupLayout,
    pub(crate) sampler: D::Sampler,
}

impl<D: Device> TemporalPass<D> {
    pub fn new(device: &D, group0: &D::BindGroupLayout) -> Result<Self, RenderError> {
        let texture = |binding| BindGroupLayoutEntry {
            binding,
            visibility: ShaderStages::FRAGMENT,
            ty: BindingType::Texture { filterable: true },
        };
        let layout = device.create_bind_group_layout(&BindGroupLayoutDesc {
            label: "group1: temporal resolve inputs",
            entries: &[
                texture(0),
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::DepthTexture,
                },
                texture(2),
                BindGroupLayoutEntry {
                    binding: 3,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler { comparison: false },
                },
                texture(4),
            ],
        });
        let sampler = device.create_sampler(&SamplerDesc {
            label: "temporal history bilinear sampler",
            ..SamplerDesc::default()
        });
        let shader = device.create_shader_module(&ShaderModuleDesc {
            label: "temporal resolve",
            wgsl: pdviewx_shaders::TEMPORAL_RESOLVE,
        })?;
        let pipeline = device.create_render_pipeline(&RenderPipelineDesc {
            label: "temporal HDR resolve",
            layouts: &[Some(group0), Some(&layout)],
            shader: &shader,
            vs_entry: "vs_fullscreen",
            fs_entry: Some("fs_temporal_resolve"),
            color_targets: &[ColorTarget {
                format: TextureFormat::Rgba16Float,
                blend: pdviewx_gpu::BlendMode::Replace,
            }],
            depth: None,
            constants: &[],
            topology: PrimitiveTopology::TriangleList,
        })?;
        Ok(Self {
            pipeline,
            layout,
            sampler,
        })
    }

    pub fn record(ctx: &mut PassContext<'_, D>) {
        let target_id = if ctx.temporal_write == 0 {
            HISTORY_A_RESOURCE
        } else {
            HISTORY_B_RESOURCE
        };
        let Some(target) = ctx.resources.view(target_id) else {
            return;
        };
        let Some(FrameBindings { temporal, .. }) = ctx.bindings else {
            return;
        };
        let source = usize::from(ctx.scene.has_translucency());
        let Some(bindings) = temporal
            .get(source)
            .and_then(|bindings| bindings.get(ctx.temporal_write))
        else {
            return;
        };
        let mut pass = ctx.encoder.begin_render_pass(&RenderPassDesc {
            label: "temporal HDR resolve",
            colors: &[ColorAttachment {
                view: target,
                load: LoadOp::Clear([0.0, 0.0, 0.0, 0.0]),
            }],
            depth: None,
            timestamps: ctx.timestamps,
        });
        pass.set_pipeline(&ctx.passes.temporal.pipeline);
        pass.set_bind_group(0, &ctx.scene.group0, &[]);
        pass.set_bind_group(1, bindings, &[]);
        pass.draw(0..3, 0..1);
    }
}
