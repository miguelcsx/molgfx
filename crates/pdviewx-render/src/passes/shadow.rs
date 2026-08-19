//! Scene-fit depth rendering for direct molecular shadows.

use crate::error::RenderError;
use crate::graph::PassContext;
use crate::passes::SHADOW_RESOURCE;
use pdviewx_gpu::{
    CommandEncoder as _, CompareFunction, DepthAttachment, DepthLoadOp, DepthState, Device,
    PrimitiveTopology, RenderPassDesc, RenderPassEncoder as _, RenderPipelineDesc,
    ShaderModuleDesc, TextureFormat,
};

/// The fixed shadow-map pipelines for analytic atoms and bonds.
#[derive(Debug)]
pub struct ShadowPass<D: Device> {
    pub(crate) sphere: D::Pipeline,
    pub(crate) bond: D::Pipeline,
    pub(crate) ribbon: D::Pipeline,
    pub(crate) primitive: D::Pipeline,
}

impl<D: Device> ShadowPass<D> {
    /// Compiles the depth-only analytic primitive pipelines.
    ///
    /// # Errors
    ///
    /// Returns a shader or pipeline construction error.
    pub fn new(
        device: &D,
        group0: &D::BindGroupLayout,
        group2: &D::BindGroupLayout,
        ribbon: &D::BindGroupLayout,
        primitive: &D::BindGroupLayout,
    ) -> Result<Self, RenderError> {
        let shader = device.create_shader_module(&ShaderModuleDesc {
            label: "analytic molecular shadows",
            wgsl: pdviewx_shaders::SHADOW,
        })?;
        let depth = Some(DepthState {
            format: TextureFormat::Depth32Float,
            write: true,
            compare: CompareFunction::GreaterEqual,
        });
        let sphere = device.create_render_pipeline(&RenderPipelineDesc {
            label: "sphere shadow map",
            layouts: &[Some(group0), None, Some(group2)],
            shader: &shader,
            vs_entry: "vs_shadow_sphere",
            fs_entry: Some("fs_shadow_sphere"),
            color_targets: &[],
            depth,
            constants: &[],
            topology: PrimitiveTopology::TriangleList,
        })?;
        let bond = device.create_render_pipeline(&RenderPipelineDesc {
            label: "bond shadow map",
            layouts: &[Some(group0), None, Some(group2)],
            shader: &shader,
            vs_entry: "vs_shadow_bond",
            fs_entry: Some("fs_shadow_bond"),
            color_targets: &[],
            depth,
            constants: &[],
            topology: PrimitiveTopology::TriangleList,
        })?;
        let ribbon_shader = device.create_shader_module(&ShaderModuleDesc {
            label: "cartoon ribbon shadows",
            wgsl: pdviewx_shaders::SHADOW_RIBBON,
        })?;
        let ribbon = device.create_render_pipeline(&RenderPipelineDesc {
            label: "ribbon shadow map",
            layouts: &[Some(group0), None, Some(ribbon)],
            shader: &ribbon_shader,
            vs_entry: "vs_shadow_ribbon",
            fs_entry: None,
            color_targets: &[],
            depth,
            constants: &[],
            topology: PrimitiveTopology::TriangleList,
        })?;
        let primitive = device.create_render_pipeline(&RenderPipelineDesc {
            label: "primitive shadow map",
            layouts: &[Some(group0), None, Some(primitive)],
            shader: &shader,
            vs_entry: "vs_shadow_primitive",
            fs_entry: Some("fs_shadow_primitive"),
            color_targets: &[],
            depth,
            constants: &[],
            topology: PrimitiveTopology::TriangleList,
        })?;
        Ok(Self {
            sphere,
            bond,
            ribbon,
            primitive,
        })
    }

    /// Records one depth pass over the GPU-cull-selected opaque streams.
    pub fn record(ctx: &mut PassContext<'_, D>) {
        if ctx.scene.is_massive_points_only() {
            return;
        }
        let Some(depth) = ctx.resources.view(SHADOW_RESOURCE) else {
            return;
        };
        let mut pass = ctx.encoder.begin_render_pass(&RenderPassDesc {
            label: "scene-fit analytic shadows",
            colors: &[],
            depth: Some(DepthAttachment {
                view: depth,
                load: DepthLoadOp::Clear(0.0),
                read_only: false,
            }),
            timestamps: ctx.timestamps,
        });
        pass.set_bind_group(0, &ctx.scene.group0, &[]);
        pass.set_pipeline(&ctx.passes.shadow.sphere);
        for (group, args, _) in ctx.scene.shadow_atom_draws(ctx.quality) {
            pass.set_bind_group(2, group, &[]);
            pass.draw_indirect(args, 0);
        }
        pass.set_pipeline(&ctx.passes.shadow.bond);
        for (group, args, _) in ctx.scene.shadow_bond_draws(ctx.quality) {
            pass.set_bind_group(2, group, &[]);
            pass.draw_indirect(args, 0);
        }
        pass.set_pipeline(&ctx.passes.shadow.ribbon);
        for (group, args, _) in ctx.scene.cartoon_draws(false) {
            pass.set_bind_group(2, group, &[]);
            pass.draw_indirect(args, 0);
        }
        for (group, args, _) in ctx.scene.mesh_draws(false) {
            pass.set_bind_group(2, group, &[]);
            pass.draw_indirect(args, 0);
        }
        pass.set_pipeline(&ctx.passes.shadow.primitive);
        if let Some((group, args)) = ctx.scene.primitive_shadow_draw(ctx.quality) {
            pass.set_bind_group(2, group, &[]);
            pass.draw_indirect(args, 0);
        }
    }
}
