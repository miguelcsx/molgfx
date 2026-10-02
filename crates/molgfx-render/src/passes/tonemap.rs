//! HDR tonemapping into the caller-owned presentation target.

use crate::engine::{DisplayGamut, TransferFunction};
use crate::error::RenderError;
use crate::graph::{DisplayEncoding, PassContext, PassEnv, ResourceId};
use crate::passes::{FrameBindings, Lazy};
use molgfx_gpu::{
    BindGroupLayoutDesc, BindGroupLayoutEntry, BindingType, ColorAttachment, ColorTarget,
    CommandEncoder as _, Device, LoadOp, PrimitiveTopology, RenderPassDesc, RenderPassEncoder as _,
    RenderPipelineDesc, ShaderModuleDesc, ShaderStages, TextureFormat,
};

/// One pipeline per display encoding.
///
/// The shader takes the gamut and transfer curve as pipeline constants, so the
/// fragment stage encodes for exactly one output instead of branching through
/// every curve on every pixel. The encoding set is closed and small; a variant
/// is built the first frame that presents with it, and a session presents with
/// one or two of the twenty-four.
#[derive(Debug)]
pub(crate) struct TonemapPass<D: Device> {
    shader: D::ShaderModule,
    target_format: TextureFormat,
    variants: Vec<Lazy<D::Pipeline>>,
    pub(crate) layout: D::BindGroupLayout,
}

/// Number of pipelines built for one anti-aliasing setting.
const ENCODINGS: usize = DisplayGamut::ALL.len() * TransferFunction::ALL.len();

/// Position of one encoding and anti-aliasing setting in
/// [`TonemapPass::variants`]: every plain encoding first, then the same
/// encodings with edge smoothing fused in.
const fn variant_index(encoding: DisplayEncoding, smoothed: bool) -> usize {
    let plain = encoding.gamut.index() * TransferFunction::ALL.len() + encoding.transfer.index();
    if smoothed { ENCODINGS + plain } else { plain }
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
        let variants = (0..2 * ENCODINGS).map(|_| Lazy::default()).collect();
        Ok(Self {
            shader,
            target_format,
            variants,
            layout,
        })
    }

    fn build_variant(
        &self,
        env: &PassEnv<'_, D>,
        encoding: DisplayEncoding,
        smoothed: bool,
    ) -> Result<D::Pipeline, RenderError> {
        Ok(env.device.create_render_pipeline(&RenderPipelineDesc {
            label: "HDR tonemap",
            layouts: &[Some(&env.scene.group0_layout), Some(&self.layout)],
            shader: &self.shader,
            vs_entry: "vs_fullscreen",
            fs_entry: Some("fs_tonemap"),
            color_targets: &[ColorTarget {
                format: self.target_format,
                blend: molgfx_gpu::BlendMode::Replace,
            }],
            depth: None,
            constants: &[
                ("PRESENTATION_GAMUT_TAG", f64::from(encoding.gamut.tag())),
                (
                    "PRESENTATION_TRANSFER_TAG",
                    f64::from(encoding.transfer.tag()),
                ),
                ("FXAA_ENABLED", f64::from(u8::from(smoothed))),
            ],
            topology: PrimitiveTopology::TriangleList,
        })?)
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
        let env = ctx.env();
        let mut pass = ctx.encoder.begin_render_pass(&RenderPassDesc {
            label: "HDR tonemap",
            colors: &[ColorAttachment {
                view: target,
                load: LoadOp::Clear([0.0, 0.0, 0.0, 1.0]),
            }],
            depth: None,
            timestamps: ctx.timestamps,
        });
        let tonemap = &ctx.passes.tonemap;
        let encoding = ctx.display_encoding;
        let smoothed = ctx.edge_smoothing;
        let Some(cell) = tonemap.variants.get(variant_index(encoding, smoothed)) else {
            return;
        };
        let Some(pipeline) = cell.build_in(&env, &mut *ctx.failure, |env| {
            tonemap.build_variant(env, encoding, smoothed)
        }) else {
            return;
        };
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &ctx.scene.group0, &[]);
        pass.set_bind_group(1, bindings, &[]);
        pass.draw(0..3, 0..1);
    }
}
