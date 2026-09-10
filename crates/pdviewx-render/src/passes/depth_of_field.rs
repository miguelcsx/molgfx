//! Tile-classified thin-lens depth of field.
//!
//! Classification is `O(pixels)` with each source depth loaded once across
//! 16×16 tiles. Resolve is `O(active pixels × 32)` and exits after one mask
//! load for sharp tiles. All targets and bind groups are pooled across frames.

use crate::error::RenderError;
use crate::graph::PassContext;
use crate::passes::{DOF_RESOURCE, DOF_TILE_RESOURCE, FrameBindings};
use pdviewx_gpu::{
    BindGroupLayoutDesc, BindGroupLayoutEntry, BindingType, ColorAttachment, ColorTarget,
    CommandEncoder as _, Device, LoadOp, PrimitiveTopology, RenderPassDesc, RenderPassEncoder as _,
    RenderPipelineDesc, ShaderModuleDesc, ShaderStages, TextureFormat,
};

/// Pipelines and layouts for the classification and bounded gather stages.
#[derive(Debug)]
pub struct DepthOfFieldPass<D: Device> {
    classify_pipeline: D::Pipeline,
    resolve_pipeline: D::Pipeline,
    pub(crate) classify_layout: D::BindGroupLayout,
    pub(crate) resolve_layout: D::BindGroupLayout,
}

impl<D: Device> DepthOfFieldPass<D> {
    pub fn new(device: &D, group0: &D::BindGroupLayout) -> Result<Self, RenderError> {
        let classify_layout = device.create_bind_group_layout(&BindGroupLayoutDesc {
            label: "group1: depth-of-field tile classification",
            entries: &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::DepthTexture,
            }],
        });
        let resolve_layout = device.create_bind_group_layout(&BindGroupLayoutDesc {
            label: "group2: depth-of-field gather inputs",
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
            label: "thin-lens depth of field",
            wgsl: pdviewx_shaders::DEPTH_OF_FIELD,
        })?;
        let classify_pipeline = device.create_render_pipeline(&RenderPipelineDesc {
            label: "depth-of-field tile classification",
            layouts: &[Some(group0), Some(&classify_layout)],
            shader: &shader,
            vs_entry: "vs_fullscreen",
            fs_entry: Some("fs_classify_dof"),
            color_targets: &[ColorTarget {
                format: TextureFormat::R8Unorm,
                blend: pdviewx_gpu::BlendMode::Replace,
            }],
            depth: None,
            constants: &[],
            topology: PrimitiveTopology::TriangleList,
        })?;
        let resolve_pipeline = device.create_render_pipeline(&RenderPipelineDesc {
            label: "depth-of-field bounded gather",
            layouts: &[Some(group0), None, Some(&resolve_layout)],
            shader: &shader,
            vs_entry: "vs_fullscreen",
            fs_entry: Some("fs_resolve_dof"),
            color_targets: &[ColorTarget {
                format: TextureFormat::Rgba16Float,
                blend: pdviewx_gpu::BlendMode::Replace,
            }],
            depth: None,
            constants: &[],
            topology: PrimitiveTopology::TriangleList,
        })?;
        Ok(Self {
            classify_pipeline,
            resolve_pipeline,
            classify_layout,
            resolve_layout,
        })
    }

    pub fn classify(ctx: &mut PassContext<'_, D>) {
        let Some(effect) = &ctx.passes.depth_of_field else {
            return;
        };
        let Some(target) = ctx.resources.view(DOF_TILE_RESOURCE) else {
            return;
        };
        let Some(FrameBindings {
            dof_classify: Some(dof_classify),
            ..
        }) = ctx.bindings
        else {
            return;
        };
        let mut pass = ctx.encoder.begin_render_pass(&RenderPassDesc {
            label: "depth-of-field tile classification",
            colors: &[ColorAttachment {
                view: target,
                load: LoadOp::Clear([0.0, 0.0, 0.0, 0.0]),
            }],
            depth: None,
            timestamps: ctx.timestamps,
        });
        pass.set_pipeline(&effect.classify_pipeline);
        pass.set_bind_group(0, &ctx.scene.group0, &[]);
        pass.set_bind_group(1, dof_classify, &[]);
        pass.draw(0..3, 0..1);
    }

    pub fn resolve(ctx: &mut PassContext<'_, D>) {
        let Some(effect) = &ctx.passes.depth_of_field else {
            return;
        };
        let Some(target) = ctx.resources.view(DOF_RESOURCE) else {
            return;
        };
        let Some(FrameBindings { dof: Some(dof), .. }) = ctx.bindings else {
            return;
        };
        let Some(bindings) = dof.get(ctx.temporal_write) else {
            return;
        };
        let mut pass = ctx.encoder.begin_render_pass(&RenderPassDesc {
            label: "depth-of-field bounded gather",
            colors: &[ColorAttachment {
                view: target,
                load: LoadOp::Clear([0.0, 0.0, 0.0, 0.0]),
            }],
            depth: None,
            timestamps: ctx.timestamps,
        });
        pass.set_pipeline(&effect.resolve_pipeline);
        pass.set_bind_group(0, &ctx.scene.group0, &[]);
        pass.set_bind_group(2, bindings, &[]);
        pass.draw(0..3, 0..1);
    }
}
