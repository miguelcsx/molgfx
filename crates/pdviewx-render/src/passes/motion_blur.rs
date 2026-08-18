//! Camera-shutter motion blur from the opaque G-buffer's motion vectors.
//!
//! The pass is a single bounded fullscreen gather. Motion is generated from
//! current and previous analytic surface positions in the geometry passes, so
//! the blur follows molecular motion and camera motion without a second scene
//! render. It runs after temporal resolve and depth of field, preserving the
//! graph's stable presentation order.

use crate::error::RenderError;
use crate::graph::PassContext;
use crate::passes::{FrameBindings, MOTION_BLUR_RESOURCE};
use pdviewx_gpu::{
    BindGroupLayoutDesc, BindGroupLayoutEntry, BindingType, ColorAttachment, ColorTarget,
    CommandEncoder as _, Device, LoadOp, PrimitiveTopology, RenderPassDesc, RenderPassEncoder as _,
    RenderPipelineDesc, ShaderModuleDesc, ShaderStages, TextureFormat,
};

/// Fullscreen motion-vector gather state.
#[derive(Debug)]
pub struct MotionBlurPass<D: Device> {
    pipeline: D::Pipeline,
    pub(crate) layout: D::BindGroupLayout,
}

impl<D: Device> MotionBlurPass<D> {
    pub fn new(device: &D, group0: &D::BindGroupLayout) -> Result<Self, RenderError> {
        let layout = device.create_bind_group_layout(&BindGroupLayoutDesc {
            label: "group1: motion blur inputs",
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture { filterable: true },
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture { filterable: true },
                },
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler { comparison: false },
                },
            ],
        });
        let shader = device.create_shader_module(&ShaderModuleDesc {
            label: "camera-shutter motion blur",
            wgsl: pdviewx_shaders::MOTION_BLUR,
        })?;
        let pipeline = device.create_render_pipeline(&RenderPipelineDesc {
            label: "camera-shutter motion blur",
            layouts: &[Some(group0), Some(&layout)],
            shader: &shader,
            vs_entry: "vs_fullscreen",
            fs_entry: Some("fs_motion_blur"),
            color_targets: &[ColorTarget {
                format: TextureFormat::Rgba16Float,
                blend: pdviewx_gpu::BlendMode::Replace,
            }],
            depth: None,
            constants: &[],
            topology: PrimitiveTopology::TriangleList,
        })?;
        Ok(Self { pipeline, layout })
    }

    pub fn record(ctx: &mut PassContext<'_, D>) {
        let Some(target) = ctx.resources.view(MOTION_BLUR_RESOURCE) else {
            return;
        };
        let Some(FrameBindings {
            motion_blur_history,
            motion_blur_dof,
            ..
        }) = ctx.bindings
        else {
            return;
        };
        let source = if ctx.depth_of_field {
            motion_blur_dof
        } else {
            let Some(source) = motion_blur_history.get(ctx.temporal_write) else {
                return;
            };
            source
        };
        let mut pass = ctx.encoder.begin_render_pass(&RenderPassDesc {
            label: "camera-shutter motion blur",
            colors: &[ColorAttachment {
                view: target,
                load: LoadOp::Clear([0.0, 0.0, 0.0, 0.0]),
            }],
            depth: None,
            timestamps: ctx.timestamps,
        });
        pass.set_pipeline(&ctx.passes.motion_blur.pipeline);
        pass.set_bind_group(0, &ctx.scene.group0, &[]);
        pass.set_bind_group(1, source, &[]);
        pass.draw(0..3, 0..1);
    }
}
