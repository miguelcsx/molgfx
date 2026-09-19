//! Edge-aware denoise of the traced occlusion buffer.
//!
//! One fullscreen bilateral pass between occlusion and lighting. It is
//! unconditional: the traced path needs it to converge quickly, and the
//! screen-space path gains from having its fixed-tap structure smoothed
//! without crossing silhouettes.

use crate::error::RenderError;
use crate::graph::PassContext;
use crate::passes::{AO_DENOISED_RESOURCE, FrameBindings};
use molgfx_gpu::{
    BindGroupLayoutDesc, BindGroupLayoutEntry, BindingType, ColorAttachment, ColorTarget,
    CommandEncoder as _, Device, LoadOp, PrimitiveTopology, RenderPassDesc, RenderPassEncoder as _,
    RenderPipelineDesc, ShaderModuleDesc, ShaderStages,
};

/// Pipeline and layout for the occlusion denoiser.
#[derive(Debug)]
pub(crate) struct AoDenoisePass<D: Device> {
    pipeline: D::Pipeline,
    pub(crate) layout: D::BindGroupLayout,
}

impl<D: Device> AoDenoisePass<D> {
    pub(crate) fn new(device: &D, group0: &D::BindGroupLayout) -> Result<Self, RenderError> {
        let layout = device.create_bind_group_layout(&BindGroupLayoutDesc {
            label: "group1: occlusion denoise inputs",
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture { filterable: true },
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::DepthTexture,
                },
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture { filterable: true },
                },
            ],
        });
        let shader = device.create_shader_module(&ShaderModuleDesc {
            label: "occlusion denoise",
            wgsl: molgfx_shaders::AO_DENOISE,
        })?;
        let pipeline = device.create_render_pipeline(&RenderPipelineDesc {
            label: "occlusion denoise",
            layouts: &[Some(group0), Some(&layout)],
            shader: &shader,
            vs_entry: "vs_fullscreen",
            fs_entry: Some("fs_denoise_occlusion"),
            color_targets: &[ColorTarget {
                format: crate::passes::AO_FORMAT,
                blend: molgfx_gpu::BlendMode::Replace,
            }],
            depth: None,
            constants: &[],
            topology: PrimitiveTopology::TriangleList,
        })?;
        Ok(Self { pipeline, layout })
    }

    pub(crate) fn record(ctx: &mut PassContext<'_, D>) {
        if ctx.scene.is_massive_points_only() {
            return;
        }
        let Some(target) = ctx.resources.view(AO_DENOISED_RESOURCE) else {
            return;
        };
        let Some(FrameBindings { ao_denoise, .. }) = ctx.bindings else {
            return;
        };
        let mut pass = ctx.encoder.begin_render_pass(&RenderPassDesc {
            label: "occlusion denoise",
            colors: &[ColorAttachment {
                view: target,
                load: LoadOp::Clear([1.0, 1.0, 0.0, 0.0]),
            }],
            depth: None,
            timestamps: ctx.timestamps,
        });
        pass.set_pipeline(&ctx.passes.ao_denoise.pipeline);
        pass.set_bind_group(0, &ctx.scene.group0, &[]);
        pass.set_bind_group(1, ao_denoise, &[]);
        pass.draw(0..3, 0..1);
    }
}
