//! Scene-fit depth rendering for direct molecular shadows.

use crate::error::RenderError;
use crate::graph::PassContext;
use crate::passes::SHADOW_RESOURCE;
use crate::passes::ligand_pose_pipelines::LigandPosePipelineSet;
use crate::passes::primitive_pipelines::PRIMITIVE_QUAD_VERTICES;
use crate::passes::primitive_shadow_pipelines::PrimitiveShadowPipelineSet;
use crate::passes::visual_pipelines::{VisualPipelineSet, constants};
use crate::scene_gpu::DrawFamily;
use molgfx_gpu::{
    CommandEncoder as _, CompareFunction, DepthAttachment, DepthLoadOp, DepthState, Device,
    PrimitiveTopology, RenderPassDesc, RenderPassEncoder as _, RenderPipelineDesc,
    ShaderModuleDesc, TextureFormat,
};

/// The fixed shadow-map pipelines for analytic atoms and bonds.
#[derive(Debug)]
pub(crate) struct ShadowPass<D: Device> {
    sphere: VisualPipelineSet<D>,
    bond: VisualPipelineSet<D>,
    ribbon: VisualPipelineSet<D>,
    primitive: PrimitiveShadowPipelineSet<D>,
    ligand_pose: LigandPosePipelineSet<D>,
}

impl<D: Device> ShadowPass<D> {
    /// Compiles the depth-only analytic primitive pipelines.
    ///
    /// # Errors
    ///
    /// Returns a shader or pipeline construction error.
    pub(crate) fn new(
        device: &D,
        group0: &D::BindGroupLayout,
        group2: &D::BindGroupLayout,
        ribbon: &D::BindGroupLayout,
        primitive: &D::BindGroupLayout,
        ligand_pose: &D::BindGroupLayout,
    ) -> Result<Self, RenderError> {
        let shader = device.create_shader_module(&ShaderModuleDesc {
            label: "analytic molecular shadows",
            wgsl: molgfx_shaders::SHADOW,
        })?;
        let depth = Some(DepthState {
            format: TextureFormat::Depth32Float,
            write: true,
            compare: CompareFunction::GreaterEqual,
        });
        let visual_pipeline =
            |label, vertex, fragment| -> Result<VisualPipelineSet<D>, RenderError> {
                let build = |pipeline_constants: &[(&'static str, f64)]| {
                    device.create_render_pipeline(&RenderPipelineDesc {
                        label,
                        layouts: &[Some(group0), None, Some(group2)],
                        shader: &shader,
                        vs_entry: vertex,
                        fs_entry: Some(fragment),
                        color_targets: &[],
                        depth,
                        constants: pipeline_constants,
                        topology: PrimitiveTopology::TriangleList,
                    })
                };
                Ok(VisualPipelineSet::new(
                    build(&[])?,
                    build(&constants(false))?,
                    build(&constants(true))?,
                ))
            };
        let sphere = visual_pipeline("sphere shadow map", "vs_shadow_sphere", "fs_shadow_sphere")?;
        let bond = visual_pipeline("bond shadow map", "vs_shadow_bond", "fs_shadow_bond")?;
        let ribbon_shader = device.create_shader_module(&ShaderModuleDesc {
            label: "cartoon ribbon shadows",
            wgsl: molgfx_shaders::SHADOW_RIBBON,
        })?;
        let ribbon_pipeline = |fragment, pipeline_constants: &[(&'static str, f64)]| {
            device.create_render_pipeline(&RenderPipelineDesc {
                label: "ribbon shadow map",
                layouts: &[Some(group0), None, Some(ribbon)],
                shader: &ribbon_shader,
                vs_entry: "vs_shadow_ribbon",
                fs_entry: fragment,
                color_targets: &[],
                depth,
                constants: pipeline_constants,
                topology: PrimitiveTopology::TriangleList,
            })
        };
        let ribbon = VisualPipelineSet::new(
            ribbon_pipeline(None, &[])?,
            ribbon_pipeline(Some("fs_shadow_ribbon"), &constants(false))?,
            ribbon_pipeline(Some("fs_shadow_ribbon"), &constants(true))?,
        );
        let primitive = PrimitiveShadowPipelineSet::new(device, group0, primitive, depth)?;
        let ligand_pose = LigandPosePipelineSet::shadow(device, group0, ligand_pose, depth)?;
        Ok(Self {
            sphere,
            bond,
            ribbon,
            primitive,
            ligand_pose,
        })
    }

    /// Compiles an analytic shadow pipeline built from the generated sibling.
    ///
    /// The sphere and bond units share one source but no entry point, so each
    /// is built from its own call rather than one shared family.
    ///
    /// # Errors
    ///
    /// Shader compilation or pipeline creation failed.
    pub(crate) fn build_specialized(
        device: &D,
        group0: &D::BindGroupLayout,
        group2: &D::BindGroupLayout,
        bond: bool,
    ) -> Result<D::Pipeline, RenderError> {
        let (label, vertex, fragment) = if bond {
            (
                "specialized bond shadow map",
                "vs_shadow_bond",
                "fs_shadow_bond",
            )
        } else {
            (
                "specialized sphere shadow map",
                "vs_shadow_sphere",
                "fs_shadow_sphere",
            )
        };
        let shader = device.create_shader_module(&ShaderModuleDesc {
            label: "analytic molecular shadows generated",
            wgsl: molgfx_shaders::SHADOW_SPECIALIZED,
        })?;
        Ok(device.create_render_pipeline(&RenderPipelineDesc {
            label,
            layouts: &[Some(group0), None, Some(group2)],
            shader: &shader,
            vs_entry: vertex,
            fs_entry: Some(fragment),
            color_targets: &[],
            depth: Some(DepthState {
                format: TextureFormat::Depth32Float,
                write: true,
                compare: CompareFunction::GreaterEqual,
            }),
            constants: &constants(true),
            topology: PrimitiveTopology::TriangleList,
        })?)
    }

    /// Compiles the ribbon shadow pipeline built from the generated sibling.
    ///
    /// # Errors
    ///
    /// Shader compilation or pipeline creation failed.
    pub(crate) fn build_specialized_ribbon(
        device: &D,
        group0: &D::BindGroupLayout,
        ribbon: &D::BindGroupLayout,
    ) -> Result<D::Pipeline, RenderError> {
        let shader = device.create_shader_module(&ShaderModuleDesc {
            label: "cartoon ribbon shadows generated",
            wgsl: molgfx_shaders::SHADOW_RIBBON_SPECIALIZED,
        })?;
        Ok(device.create_render_pipeline(&RenderPipelineDesc {
            label: "specialized ribbon shadow map",
            layouts: &[Some(group0), None, Some(ribbon)],
            shader: &shader,
            vs_entry: "vs_shadow_ribbon",
            fs_entry: Some("fs_shadow_ribbon"),
            color_targets: &[],
            depth: Some(DepthState {
                format: TextureFormat::Depth32Float,
                write: true,
                compare: CompareFunction::GreaterEqual,
            }),
            constants: &constants(true),
            topology: PrimitiveTopology::TriangleList,
        })?)
    }

    /// Records one depth pass over the GPU-cull-selected opaque streams.
    pub(crate) fn record(ctx: &mut PassContext<'_, D>) {
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
        let mut bound: Option<*const D::Pipeline> = None;
        if let Some(arena) = ctx.scene.indirect_args() {
            for (group, offset, shading, specialized) in ctx.scene.shadow_atom_draws(ctx.quality) {
                let pipeline = ctx.passes.shadow.sphere.select(shading, specialized);
                if bound != Some(std::ptr::from_ref(pipeline)) {
                    pass.set_pipeline(pipeline);
                    bound = Some(std::ptr::from_ref(pipeline));
                }
                pass.set_bind_group(2, group, &[]);
                pass.draw_indirect(arena, offset);
            }
        }
        bound = None;
        if let Some(arena) = ctx.scene.indirect_args() {
            for (group, offset, shading, specialized) in ctx.scene.shadow_bond_draws(ctx.quality) {
                let pipeline = ctx.passes.shadow.bond.select(shading, specialized);
                if bound != Some(std::ptr::from_ref(pipeline)) {
                    pass.set_pipeline(pipeline);
                    bound = Some(std::ptr::from_ref(pipeline));
                }
                pass.set_bind_group(2, group, &[]);
                pass.draw_indirect(arena, offset);
            }
        }
        bound = None;
        for (group, args, shading, specialized) in ctx
            .scene
            .cartoon_draws(false, DrawFamily::ShadowRibbon)
            .chain(ctx.scene.mesh_draws(false))
        {
            let pipeline = ctx.passes.shadow.ribbon.select(shading, specialized);
            if bound != Some(std::ptr::from_ref(pipeline)) {
                pass.set_pipeline(pipeline);
                bound = Some(std::ptr::from_ref(pipeline));
            }
            pass.set_bind_group(2, group, &[]);
            pass.draw_indirect(args, 0);
        }
        if let Some((group, runs)) = ctx.scene.primitive_shadow_draw(ctx.quality) {
            pass.set_bind_group(2, group, &[]);
            for run in runs {
                let Some(pipeline) = ctx.passes.shadow.primitive.pipeline(run) else {
                    continue;
                };
                pass.set_pipeline(pipeline);
                pass.draw(0..PRIMITIVE_QUAD_VERTICES, run.first..run.first + run.len);
            }
        }
        if let Some((group, args, runs)) = ctx.scene.ligand_pose_shadow_draws(ctx.quality) {
            pass.set_bind_group(2, group, &[]);
            for run in runs.iter().filter(|run| !run.translucent) {
                let Some(pipeline) = ctx.passes.shadow.ligand_pose.pipeline(run) else {
                    continue;
                };
                pass.set_pipeline(pipeline);
                pass.draw_indirect(args, run.args_offset);
            }
        }
    }
}
