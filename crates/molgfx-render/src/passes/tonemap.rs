//! HDR tonemapping into the caller-owned presentation target.

use crate::engine::{DisplayGamut, TransferFunction};
use crate::error::RenderError;
use crate::graph::{DisplayEncoding, PassContext, ResourceId};
use crate::passes::FrameBindings;
use molgfx_gpu::{
    BindGroupLayoutDesc, BindGroupLayoutEntry, BindingType, ColorAttachment, ColorTarget,
    CommandEncoder as _, Device, LoadOp, PrimitiveTopology, RenderPassDesc, RenderPassEncoder as _,
    RenderPipelineDesc, ShaderModuleDesc, ShaderStages, TextureFormat,
};

/// One pipeline per display encoding.
///
/// The shader takes the gamut and transfer curve as pipeline constants, so the
/// fragment stage encodes for exactly one output instead of branching through
/// every curve on every pixel. The encoding set is closed and small, so every
/// variant is built at load and none is ever created inside the frame loop.
#[derive(Debug)]
pub(crate) struct TonemapPass<D: Device> {
    variants: Vec<D::Pipeline>,
    pub(crate) layout: D::BindGroupLayout,
}

/// Position of one encoding in [`TonemapPass::variants`].
const fn variant_index(encoding: DisplayEncoding) -> usize {
    encoding.gamut.index() * TransferFunction::ALL.len() + encoding.transfer.index()
}

impl<D: Device> TonemapPass<D> {
    pub(crate) fn new(
        device: &D,
        target_format: TextureFormat,
        group0: &D::BindGroupLayout,
    ) -> Result<Self, RenderError> {
        let layout = device.create_bind_group_layout(&BindGroupLayoutDesc {
            label: "group1: tonemap input",
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
                BindGroupLayoutEntry {
                    binding: 3,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture { filterable: true },
                },
            ],
        });
        let shader = device.create_shader_module(&ShaderModuleDesc {
            label: "tonemap",
            wgsl: molgfx_shaders::TONEMAP,
        })?;
        let mut variants =
            Vec::with_capacity(DisplayGamut::ALL.len() * TransferFunction::ALL.len());
        for gamut in DisplayGamut::ALL {
            for transfer in TransferFunction::ALL {
                variants.push(device.create_render_pipeline(&RenderPipelineDesc {
                    label: "HDR tonemap",
                    layouts: &[Some(group0), Some(&layout)],
                    shader: &shader,
                    vs_entry: "vs_fullscreen",
                    fs_entry: Some("fs_tonemap"),
                    color_targets: &[ColorTarget {
                        format: target_format,
                        blend: molgfx_gpu::BlendMode::Replace,
                    }],
                    depth: None,
                    constants: &[
                        ("PRESENTATION_GAMUT_TAG", f64::from(gamut.tag())),
                        ("PRESENTATION_TRANSFER_TAG", f64::from(transfer.tag())),
                    ],
                    topology: PrimitiveTopology::TriangleList,
                })?);
            }
        }
        Ok(Self { variants, layout })
    }

    pub(crate) fn record(ctx: &mut PassContext<'_, D>) {
        let Some(target) = ctx.resources.view(ResourceId::SWAPCHAIN) else {
            return;
        };
        let Some(FrameBindings { tonemap, .. }) = ctx.bindings else {
            return;
        };
        let Some(bindings) = tonemap.get(ctx.temporal_write) else {
            return;
        };
        let mut pass = ctx.encoder.begin_render_pass(&RenderPassDesc {
            label: "HDR tonemap",
            colors: &[ColorAttachment {
                view: target,
                load: LoadOp::Clear([0.0, 0.0, 0.0, 1.0]),
            }],
            depth: None,
            timestamps: ctx.timestamps,
        });
        let Some(pipeline) = ctx
            .passes
            .tonemap
            .variants
            .get(variant_index(ctx.display_encoding))
        else {
            return;
        };
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &ctx.scene.group0, &[]);
        pass.set_bind_group(1, bindings, &[]);
        pass.draw(0..3, 0..1);
    }
}
