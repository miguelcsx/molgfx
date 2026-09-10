//! Specialized shadow pipelines for heterogeneous analytic primitives.

use crate::error::RenderError;
use crate::scene_gpu::{
    FAMILY_BOX, FAMILY_ELLIPSOID, FAMILY_PARTICLE, FAMILY_POLYGON, POLYGON_HEXAGON,
    POLYGON_PENTAGON, PrimitiveDrawGroup,
};
use pdviewx_gpu::{DepthState, Device, PrimitiveTopology, RenderPipelineDesc, ShaderModuleDesc};

const SHADOW_KIND_CONSTANT: &str = "SHADOW_PRIMITIVE_KIND";
const PARTICLE_SPHERE: u32 = 0;
const PARTICLE_CYLINDER: u32 = 2;
const PARTICLE_SPHEROCYLINDER: u32 = 3;
#[cfg(test)]
const PARTICLE_GAUSSIAN: u32 = 4;
const PARTICLE_CIRCLE: u32 = 5;
const PARTICLE_SQUARE: u32 = 6;
const PARTICLE_SUPERQUADRIC: u32 = 7;

const KIND_ELLIPSOID: u32 = 0;
const KIND_BOX: u32 = 1;
const KIND_SPHERE: u32 = 2;
const KIND_CYLINDER: u32 = 3;
const KIND_SPHEROCYLINDER: u32 = 4;
const KIND_POLYGON_PENTAGON: u32 = 5;
const KIND_POLYGON_HEXAGON: u32 = 6;
const KIND_CIRCLE: u32 = 7;
const KIND_SQUARE: u32 = 8;
const KIND_SUPERQUADRIC: u32 = 9;

const VARIANTS: [(u32, &str); 10] = [
    (KIND_ELLIPSOID, "ellipsoid primitive shadows"),
    (KIND_BOX, "box primitive shadows"),
    (KIND_SPHERE, "sphere primitive shadows"),
    (KIND_CYLINDER, "cylinder primitive shadows"),
    (KIND_SPHEROCYLINDER, "spherocylinder primitive shadows"),
    (KIND_POLYGON_PENTAGON, "pentagon primitive shadows"),
    (KIND_POLYGON_HEXAGON, "hexagon primitive shadows"),
    (KIND_CIRCLE, "circle primitive shadows"),
    (KIND_SQUARE, "square primitive shadows"),
    (KIND_SUPERQUADRIC, "superquadric primitive shadows"),
];

#[derive(Debug)]
pub(crate) struct PrimitiveShadowPipelineSet<D: Device> {
    pipelines: Vec<D::Pipeline>,
}

impl<D: Device> PrimitiveShadowPipelineSet<D> {
    pub(crate) fn new(
        device: &D,
        group0: &D::BindGroupLayout,
        primitive: &D::BindGroupLayout,
        depth: Option<DepthState>,
    ) -> Result<Self, RenderError> {
        let shader = device.create_shader_module(&ShaderModuleDesc {
            label: "heterogeneous analytic primitive shadows",
            wgsl: pdviewx_shaders::SHADOW,
        })?;
        let mut pipelines = Vec::with_capacity(VARIANTS.len());
        for (kind, label) in VARIANTS {
            pipelines.push(device.create_render_pipeline(&RenderPipelineDesc {
                label,
                layouts: &[Some(group0), None, Some(primitive)],
                shader: &shader,
                vs_entry: "vs_shadow_primitive",
                fs_entry: Some("fs_shadow_primitive"),
                color_targets: &[],
                depth,
                constants: &[(SHADOW_KIND_CONSTANT, f64::from(kind))],
                topology: PrimitiveTopology::TriangleList,
            })?);
        }
        Ok(Self { pipelines })
    }

    pub(crate) fn pipeline(&self, group: &PrimitiveDrawGroup) -> Option<&D::Pipeline> {
        let index = usize::try_from(shadow_kind(group)?).ok()?;
        self.pipelines.get(index)
    }
}

fn shadow_kind(group: &PrimitiveDrawGroup) -> Option<u32> {
    if group.translucent {
        return None;
    }
    match group.family {
        FAMILY_ELLIPSOID => Some(KIND_ELLIPSOID),
        FAMILY_BOX => Some(KIND_BOX),
        FAMILY_POLYGON => match group.shape {
            POLYGON_PENTAGON => Some(KIND_POLYGON_PENTAGON),
            POLYGON_HEXAGON => Some(KIND_POLYGON_HEXAGON),
            _ => None,
        },
        FAMILY_PARTICLE => match group.shape {
            PARTICLE_SPHERE => Some(KIND_SPHERE),
            PARTICLE_CYLINDER => Some(KIND_CYLINDER),
            PARTICLE_SPHEROCYLINDER => Some(KIND_SPHEROCYLINDER),
            PARTICLE_CIRCLE => Some(KIND_CIRCLE),
            PARTICLE_SQUARE => Some(KIND_SQUARE),
            PARTICLE_SUPERQUADRIC => Some(KIND_SUPERQUADRIC),
            _ => None,
        },
        _ => None,
    }
}

#[cfg(test)]
#[path = "primitive_shadow_pipelines_tests.rs"]
mod tests;
