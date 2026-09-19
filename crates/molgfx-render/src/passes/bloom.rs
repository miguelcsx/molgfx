//! Filmic bloom: bright-pass downsample and a separable Gaussian blur.
//!
//! All three stages share one single-texture layout and run at a quarter of the
//! frame, so a wide highlight bleed costs a sixteenth of the full-resolution
//! taps. Targets and bind groups are pooled across frames like every other
//! presentation stage.

use crate::error::RenderError;
use crate::graph::PassContext;
use crate::passes::{BLOOM_A_RESOURCE, BLOOM_B_RESOURCE, BLOOM_C_RESOURCE, FrameBindings};
use molgfx_gpu::{
    BindGroupLayoutDesc, BindGroupLayoutEntry, BindingType, ColorAttachment, ColorTarget,
    CommandEncoder as _, Device, LoadOp, PrimitiveTopology, RenderPassDesc, RenderPassEncoder as _,
    RenderPipelineDesc, ShaderModuleDesc, ShaderStages, TextureFormat,
};

/// Pipelines and the shared single-source layout for the three bloom stages.
#[derive(Debug)]
pub(crate) struct BloomPass<D: Device> {
    bright_pipeline: D::Pipeline,
    horizontal_pipeline: D::Pipeline,
    vertical_pipeline: D::Pipeline,
    pub(crate) layout: D::BindGroupLayout,
}

impl<D: Device> BloomPass<D> {
    pub(crate) fn new(device: &D, group0: &D::BindGroupLayout) -> Result<Self, RenderError> {
        let layout = device.create_bind_group_layout(&BindGroupLayoutDesc {
            label: "group1: bloom source",
            entries: &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::FRAGMENT,
                ty: BindingType::Texture { filterable: true },
            }],
        });
        let shader = device.create_shader_module(&ShaderModuleDesc {
            label: "bloom",
            wgsl: molgfx_shaders::BLOOM,
        })?;
        let stage = |label: &'static str, entry: &'static str| {
            device.create_render_pipeline(&RenderPipelineDesc {
                label,
                layouts: &[Some(group0), Some(&layout)],
                shader: &shader,
                vs_entry: "vs_fullscreen",
                fs_entry: Some(entry),
                color_targets: &[ColorTarget {
                    format: TextureFormat::Rgba16Float,
                    blend: molgfx_gpu::BlendMode::Replace,
                }],
                depth: None,
                constants: &[],
                topology: PrimitiveTopology::TriangleList,
            })
        };
        let bright_pipeline = stage("bloom bright pass", "fs_bloom_bright")?;
        let horizontal_pipeline = stage("bloom horizontal blur", "fs_bloom_horizontal")?;
        let vertical_pipeline = stage("bloom vertical blur", "fs_bloom_vertical")?;
        Ok(Self {
            bright_pipeline,
            horizontal_pipeline,
            vertical_pipeline,
            layout,
        })
    }

    /// Extracts the above-threshold energy of the resolved frame into the
    /// quarter-resolution ping target.
    pub(crate) fn bright(ctx: &mut PassContext<'_, D>) {
        let Some(FrameBindings {
            bloom_source: Some(bloom_source),
            ..
        }) = ctx.bindings
        else {
            return;
        };
        let Some(source) = bloom_source.get(ctx.temporal_write) else {
            return;
        };
        Self::draw(
            ctx,
            BLOOM_A_RESOURCE,
            "bloom bright pass",
            |passes| passes.bloom.as_ref().map(|effect| &effect.bright_pipeline),
            source,
        );
    }

    pub(crate) fn horizontal(ctx: &mut PassContext<'_, D>) {
        let Some(FrameBindings {
            bloom_horizontal_source: Some(bloom_horizontal_source),
            ..
        }) = ctx.bindings
        else {
            return;
        };
        Self::draw(
            ctx,
            BLOOM_B_RESOURCE,
            "bloom horizontal blur",
            |passes| {
                passes
                    .bloom
                    .as_ref()
                    .map(|effect| &effect.horizontal_pipeline)
            },
            bloom_horizontal_source,
        );
    }

    pub(crate) fn vertical(ctx: &mut PassContext<'_, D>) {
        let Some(FrameBindings {
            bloom_vertical_source: Some(bloom_vertical_source),
            ..
        }) = ctx.bindings
        else {
            return;
        };
        Self::draw(
            ctx,
            BLOOM_C_RESOURCE,
            "bloom vertical blur",
            |passes| {
                passes
                    .bloom
                    .as_ref()
                    .map(|effect| &effect.vertical_pipeline)
            },
            bloom_vertical_source,
        );
    }

    fn draw(
        ctx: &mut PassContext<'_, D>,
        target: crate::graph::ResourceId,
        label: &'static str,
        pipeline: fn(&crate::passes::PassRegistry<D>) -> Option<&D::Pipeline>,
        source: &D::BindGroup,
    ) {
        let Some(view) = ctx.resources.view(target) else {
            return;
        };
        let Some(selected) = pipeline(ctx.passes) else {
            return;
        };
        let mut pass = ctx.encoder.begin_render_pass(&RenderPassDesc {
            label,
            colors: &[ColorAttachment {
                view,
                load: LoadOp::Clear([0.0, 0.0, 0.0, 0.0]),
            }],
            depth: None,
            timestamps: ctx.timestamps,
        });
        pass.set_pipeline(selected);
        pass.set_bind_group(0, &ctx.scene.group0, &[]);
        pass.set_bind_group(1, source, &[]);
        pass.draw(0..3, 0..1);
    }
}
