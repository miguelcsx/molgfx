//! Sphere and capsule pipelines for compact ligand pose tables.

use crate::error::RenderError;
use crate::scene_gpu::{LigandPoseDrawGroup, POSE_CAPSULE, POSE_SPHERE};
use pdviewx_gpu::{
    ColorTarget, DepthState, Device, PrimitiveTopology, RenderPipelineDesc, ShaderModuleDesc,
};

const POSE_SHAPE_CONSTANT: &str = "LIGAND_POSE_SHAPE";
const POSE_TRANSLUCENT_CONSTANT: &str = "LIGAND_POSE_TRANSLUCENT";
const PARTICLE_SHAPE_CONSTANT: &str = "PARTICLE_SHAPE_KIND";
const SHADOW_KIND_CONSTANT: &str = "SHADOW_PRIMITIVE_KIND";
const SHADOW_SPHERE: f64 = 2.0;
const SHADOW_CAPSULE: f64 = 4.0;

#[derive(Debug)]
pub(crate) struct LigandPosePipelineSet<D: Device> {
    sphere: D::Pipeline,
    capsule: D::Pipeline,
}

impl<D: Device> LigandPosePipelineSet<D> {
    pub(crate) fn geometry(
        device: &D,
        group0: &D::BindGroupLayout,
        group1: Option<&D::BindGroupLayout>,
        ligand_pose: &D::BindGroupLayout,
        translucent: bool,
        targets: &[ColorTarget],
        depth: Option<DepthState>,
    ) -> Result<Self, RenderError> {
        let shader = device.create_shader_module(&ShaderModuleDesc {
            label: "compact ligand pose geometry",
            wgsl: pdviewx_shaders::GEOMETRY_LIGAND_POSE,
        })?;
        let fragment = if translucent {
            "fs_ligand_pose_transparent"
        } else {
            "fs_ligand_pose"
        };
        let build = |label, shape| {
            device.create_render_pipeline(&RenderPipelineDesc {
                label,
                layouts: &[Some(group0), group1, Some(ligand_pose)],
                shader: &shader,
                vs_entry: "vs_ligand_pose",
                fs_entry: Some(fragment),
                color_targets: targets,
                depth,
                constants: &[
                    (POSE_SHAPE_CONSTANT, f64::from(shape)),
                    (
                        POSE_TRANSLUCENT_CONSTANT,
                        if translucent { 1.0 } else { 0.0 },
                    ),
                    (PARTICLE_SHAPE_CONSTANT, f64::from(shape)),
                ],
                topology: PrimitiveTopology::TriangleList,
            })
        };
        Ok(Self {
            sphere: build("compact ligand pose spheres", POSE_SPHERE)?,
            capsule: build("compact ligand pose capsules", POSE_CAPSULE)?,
        })
    }

    pub(crate) fn shadow(
        device: &D,
        group0: &D::BindGroupLayout,
        ligand_pose: &D::BindGroupLayout,
        depth: Option<DepthState>,
    ) -> Result<Self, RenderError> {
        let shader = device.create_shader_module(&ShaderModuleDesc {
            label: "compact ligand pose shadows",
            wgsl: pdviewx_shaders::LIGAND_POSE_SHADOW,
        })?;
        let build = |label, shape, shadow_kind| {
            device.create_render_pipeline(&RenderPipelineDesc {
                label,
                layouts: &[Some(group0), None, Some(ligand_pose)],
                shader: &shader,
                vs_entry: "vs_shadow_ligand_pose",
                fs_entry: Some("fs_shadow_primitive"),
                color_targets: &[],
                depth,
                constants: &[
                    (POSE_SHAPE_CONSTANT, f64::from(shape)),
                    (POSE_TRANSLUCENT_CONSTANT, 0.0),
                    (SHADOW_KIND_CONSTANT, shadow_kind),
                ],
                topology: PrimitiveTopology::TriangleList,
            })
        };
        Ok(Self {
            sphere: build("compact ligand sphere shadows", POSE_SPHERE, SHADOW_SPHERE)?,
            capsule: build(
                "compact ligand capsule shadows",
                POSE_CAPSULE,
                SHADOW_CAPSULE,
            )?,
        })
    }

    pub(crate) fn pipeline(&self, group: &LigandPoseDrawGroup) -> Option<&D::Pipeline> {
        match group.shape {
            POSE_SPHERE => Some(&self.sphere),
            POSE_CAPSULE => Some(&self.capsule),
            _ => None,
        }
    }
}
