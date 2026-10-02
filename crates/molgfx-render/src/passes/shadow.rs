//! Scene-fit depth rendering for direct molecular shadows.

use crate::error::RenderError;
use crate::graph::{PassContext, PassEnv};
use crate::passes::ligand_pose_pipelines::LigandPosePipelineSet;
use crate::passes::primitive_pipelines::PRIMITIVE_QUAD_VERTICES;
use crate::passes::primitive_shadow_pipelines::PrimitiveShadowPipelineSet;
use crate::passes::visual_pipelines::{VisualPipelineSet, constants};
use crate::passes::{Lazy, SHADOW_RESOURCE};
use crate::scene_gpu::{DrawFamily, GpuScene};
use molgfx_gpu::{
    CommandEncoder as _, CompareFunction, DepthAttachment, DepthLoadOp, DepthState, Device,
    PrimitiveTopology, RenderPassDesc, RenderPassEncoder, RenderPipelineDesc, ShaderModuleDesc,
    TextureFormat,
};

/// The shadow-map pipelines, one family per kind of caster, each built the
/// first frame that casts a shadow of that kind.
#[derive(Debug)]
pub(crate) struct ShadowPass<D: Device> {
    sphere: Lazy<VisualPipelineSet<D>>,
    bond: Lazy<VisualPipelineSet<D>>,
    ribbon: Lazy<VisualPipelineSet<D>>,
    primitive: Lazy<PrimitiveShadowPipelineSet<D>>,
    ligand_pose: Lazy<LigandPosePipelineSet<D>>,
}

const SHADOW_DEPTH: Option<DepthState> = Some(DepthState {
    format: TextureFormat::Depth32Float,
    write: true,
    compare: CompareFunction::GreaterEqual,
});

/// The three visual variants of an analytic atom or bond shadow.
fn analytic_set<D: Device>(
    env: &PassEnv<'_, D>,
    label: &'static str,
    vertex: &'static str,
    fragment: &'static str,
) -> Result<VisualPipelineSet<D>, RenderError> {
    let shader = env.device.create_shader_module(&ShaderModuleDesc {
        label: "analytic molecular shadows",
        wgsl: molgfx_shaders::SHADOW,
    })?;
    let build = |pipeline_constants: &[(&'static str, f64)]| {
        env.device.create_render_pipeline(&RenderPipelineDesc {
            label,
            layouts: &[
                Some(&env.scene.group0_layout),
                None,
                Some(&env.scene.group2_layout),
            ],
            shader: &shader,
            vs_entry: vertex,
            fs_entry: Some(fragment),
            color_targets: &[],
            depth: SHADOW_DEPTH,
            constants: pipeline_constants,
            topology: PrimitiveTopology::TriangleList,
        })
    };
    Ok(VisualPipelineSet::new(
        build(&[])?,
        build(&constants(false))?,
        build(&constants(true))?,
    ))
}

fn ribbon_set<D: Device>(env: &PassEnv<'_, D>) -> Result<VisualPipelineSet<D>, RenderError> {
    let shader = env.device.create_shader_module(&ShaderModuleDesc {
        label: "cartoon ribbon shadows",
        wgsl: molgfx_shaders::SHADOW_RIBBON,
    })?;
    let build = |fragment, pipeline_constants: &[(&'static str, f64)]| {
        env.device.create_render_pipeline(&RenderPipelineDesc {
            label: "ribbon shadow map",
            layouts: &[
                Some(&env.scene.group0_layout),
                None,
                Some(&env.scene.ribbon_layout),
            ],
            shader: &shader,
            vs_entry: "vs_shadow_ribbon",
            fs_entry: fragment,
            color_targets: &[],
            depth: SHADOW_DEPTH,
            constants: pipeline_constants,
            topology: PrimitiveTopology::TriangleList,
        })
    };
    Ok(VisualPipelineSet::new(
        build(None, &[])?,
        build(Some("fs_shadow_ribbon"), &constants(false))?,
        build(Some("fs_shadow_ribbon"), &constants(true))?,
    ))
}

impl<D: Device> ShadowPass<D> {
    /// A shadow pass with no pipeline built yet.
    pub(crate) fn new() -> Self {
        Self {
            sphere: Lazy::default(),
            bond: Lazy::default(),
            ribbon: Lazy::default(),
            primitive: Lazy::default(),
            ligand_pose: Lazy::default(),
        }
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
        let shadow = &ctx.passes.shadow;
        let env = ctx.env();
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
        let quality = ctx.quality;
        record_analytic_casters(
            &mut pass,
            ctx.scene,
            shadow,
            &env,
            &mut *ctx.failure,
            quality,
        );
        record_primitive_casters(
            &mut pass,
            ctx.scene,
            shadow,
            &env,
            &mut *ctx.failure,
            quality,
        );
    }
}

/// Spheres, bonds and ribbons: the casters that share one pipeline-change rule.
fn record_analytic_casters<D: Device, P: RenderPassEncoder<D>>(
    pass: &mut P,
    scene: &GpuScene<D>,
    shadow: &ShadowPass<D>,
    env: &PassEnv<'_, D>,
    failure: &mut Option<RenderError>,
    quality: bool,
) {
    let mut bound: Option<*const D::Pipeline> = None;
    if let Some(arena) = scene.indirect_args() {
        for (group, offset, shading, specialized) in scene.shadow_atom_draws(quality) {
            let Some(set) = shadow.sphere.build_in(env, &mut *failure, |env| {
                analytic_set(
                    env,
                    "sphere shadow map",
                    "vs_shadow_sphere",
                    "fs_shadow_sphere",
                )
            }) else {
                break;
            };
            let pipeline = set.select(shading, specialized);
            if bound != Some(std::ptr::from_ref(pipeline)) {
                pass.set_pipeline(pipeline);
                bound = Some(std::ptr::from_ref(pipeline));
            }
            pass.set_bind_group(2, group, &[]);
            pass.draw_indirect(arena, offset);
        }
    }
    bound = None;
    if let Some(arena) = scene.indirect_args() {
        for (group, offset, shading, specialized) in scene.shadow_bond_draws(quality) {
            let Some(set) = shadow.bond.build_in(env, &mut *failure, |env| {
                analytic_set(env, "bond shadow map", "vs_shadow_bond", "fs_shadow_bond")
            }) else {
                break;
            };
            let pipeline = set.select(shading, specialized);
            if bound != Some(std::ptr::from_ref(pipeline)) {
                pass.set_pipeline(pipeline);
                bound = Some(std::ptr::from_ref(pipeline));
            }
            pass.set_bind_group(2, group, &[]);
            pass.draw_indirect(arena, offset);
        }
    }
    bound = None;
    for (group, args, shading, specialized) in scene
        .cartoon_draws(false, DrawFamily::ShadowRibbon)
        .chain(scene.mesh_draws(false))
    {
        let Some(set) = shadow.ribbon.build_in(env, &mut *failure, ribbon_set) else {
            break;
        };
        let pipeline = set.select(shading, specialized);
        if bound != Some(std::ptr::from_ref(pipeline)) {
            pass.set_pipeline(pipeline);
            bound = Some(std::ptr::from_ref(pipeline));
        }
        pass.set_bind_group(2, group, &[]);
        pass.draw_indirect(args, 0);
    }
}

/// Heterogeneous analytic primitives and ligand poses.
fn record_primitive_casters<D: Device, P: RenderPassEncoder<D>>(
    pass: &mut P,
    scene: &GpuScene<D>,
    shadow: &ShadowPass<D>,
    env: &PassEnv<'_, D>,
    failure: &mut Option<RenderError>,
    quality: bool,
) {
    if let Some((group, runs)) = scene.primitive_shadow_draw(quality) {
        pass.set_bind_group(2, group, &[]);
        for run in runs {
            let Some(set) = shadow.primitive.build_in(env, &mut *failure, |env| {
                PrimitiveShadowPipelineSet::new(
                    env.device,
                    &env.scene.group0_layout,
                    &env.scene.primitive_shadow_layout,
                    SHADOW_DEPTH,
                )
            }) else {
                break;
            };
            let Some(pipeline) = set.pipeline(run) else {
                continue;
            };
            pass.set_pipeline(pipeline);
            pass.draw(0..PRIMITIVE_QUAD_VERTICES, run.first..run.first + run.len);
        }
    }
    if let Some((group, args, runs)) = scene.ligand_pose_shadow_draws(quality) {
        pass.set_bind_group(2, group, &[]);
        for run in runs.iter().filter(|run| !run.translucent) {
            let Some(set) = shadow.ligand_pose.build_in(env, &mut *failure, |env| {
                LigandPosePipelineSet::shadow(
                    env.device,
                    &env.scene.group0_layout,
                    &env.scene.ligand_pose_layout,
                    SHADOW_DEPTH,
                )
            }) else {
                break;
            };
            let Some(pipeline) = set.pipeline(run) else {
                continue;
            };
            pass.set_pipeline(pipeline);
            pass.draw_indirect(args, run.args_offset);
        }
    }
}
