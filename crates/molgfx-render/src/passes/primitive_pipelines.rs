//! One specialized pipeline per analytic primitive class.
//!
//! Each primitive family is intersected by its own fragment routine, and the
//! particle family is specialized once more per shape through a pipeline
//! constant, so a covered pixel runs exactly one intersection instead of a
//! switch. The opaque gbuffer pass and the transparent pass hold identical
//! sets built from different fragment entries and render targets, so the class
//! catalogue and the group-to-pipeline mapping live here once rather than
//! drifting between the two passes.

use crate::error::RenderError;
use crate::scene_gpu::{
    PrimitiveDrawGroup, FAMILY_BOX, FAMILY_ELLIPSOID, FAMILY_PARTICLE, FAMILY_POLYGON,
};
use molgfx_gpu::{
    ColorTarget, DepthState, Device, PrimitiveTopology, RenderPipelineDesc, ShaderModuleDesc,
};

/// Six vertices expand one instance into a two-triangle impostor quad.
pub(crate) const PRIMITIVE_QUAD_VERTICES: u32 = 6;

/// The pipeline-constant name the particle fragment reads to pick its shape.
const PARTICLE_SHAPE_CONSTANT: &str = "PARTICLE_SHAPE_KIND";

/// The particle shapes routed through the particle family, in shape order.
///
/// Box particles are drawn by the box family, so shape 1 has no particle
/// pipeline; the rest each get one specialized by the shape constant.
const PARTICLE_SHAPE_VARIANTS: [u32; 7] = [0, 2, 3, 4, 5, 6, 7];

/// One past the largest particle shape value, sizing the pipeline table.
const PARTICLE_SHAPE_MAX: u32 = 8;

/// The four fragment entry points one variant of the primitive shader exposes.
///
/// The opaque and transparent variants differ only in these names and in the
/// render targets they write, so a caller supplies both and shares everything
/// else.
pub(crate) struct PrimitiveEntries {
    pub(crate) ellipsoid: &'static str,
    pub(crate) oriented_box: &'static str,
    pub(crate) polygon: &'static str,
    pub(crate) particle: &'static str,
}

/// The opaque gbuffer fragment entries.
pub(crate) const OPAQUE_ENTRIES: PrimitiveEntries = PrimitiveEntries {
    ellipsoid: "fs_primitive_ellipsoid",
    oriented_box: "fs_primitive_box",
    polygon: "fs_primitive_polygon",
    particle: "fs_primitive_particle",
};

/// The transparent order-independent fragment entries.
pub(crate) const TRANSPARENT_ENTRIES: PrimitiveEntries = PrimitiveEntries {
    ellipsoid: "fs_primitive_ellipsoid_transparent",
    oriented_box: "fs_primitive_box_transparent",
    polygon: "fs_primitive_polygon_transparent",
    particle: "fs_primitive_particle_transparent",
};

/// The specialized pipelines for every primitive class, plus the group-to-
/// pipeline lookup shared by both passes.
#[derive(Debug)]
pub(crate) struct PrimitivePipelineSet<D: Device> {
    ellipsoid: D::Pipeline,
    oriented_box: D::Pipeline,
    polygon: D::Pipeline,
    /// One pipeline per particle shape, indexed by shape value; unmapped
    /// shapes are absent.
    particle: Vec<Option<D::Pipeline>>,
}

impl<D: Device> PrimitivePipelineSet<D> {
    /// Builds every class pipeline against one set of fragment entries and
    /// render targets. The vertex stage and bind-group layout are shared.
    pub(crate) fn build(
        device: &D,
        group0: &D::BindGroupLayout,
        group1: Option<&D::BindGroupLayout>,
        primitive: &D::BindGroupLayout,
        entries: &PrimitiveEntries,
        color_targets: &[ColorTarget],
        depth: Option<DepthState>,
    ) -> Result<Self, RenderError> {
        let shader = device.create_shader_module(&ShaderModuleDesc {
            label: "geometry_primitive",
            wgsl: molgfx_shaders::GEOMETRY_PRIMITIVE,
        })?;
        let build = |label, fs_entry, constants: &[(&'static str, f64)]| {
            device.create_render_pipeline(&RenderPipelineDesc {
                label,
                layouts: &[Some(group0), group1, Some(primitive)],
                shader: &shader,
                vs_entry: "vs_primitive",
                fs_entry: Some(fs_entry),
                color_targets,
                depth,
                constants,
                topology: PrimitiveTopology::TriangleList,
            })
        };
        let mut particle = Vec::with_capacity(PARTICLE_SHAPE_MAX as usize);
        for shape in 0..PARTICLE_SHAPE_MAX {
            let variant = PARTICLE_SHAPE_VARIANTS.contains(&shape).then(|| {
                build(
                    "analytic particles",
                    entries.particle,
                    &[(PARTICLE_SHAPE_CONSTANT, f64::from(shape))],
                )
            });
            particle.push(variant.transpose()?);
        }
        Ok(Self {
            ellipsoid: build("analytic ellipsoids", entries.ellipsoid, &[])?,
            oriented_box: build("analytic boxes", entries.oriented_box, &[])?,
            polygon: build("analytic polygons", entries.polygon, &[])?,
            particle,
        })
    }

    /// The pipeline for one class, or `None` for an unmapped particle shape
    /// (which a well-formed table never groups).
    pub(crate) fn pipeline(&self, group: &PrimitiveDrawGroup) -> Option<&D::Pipeline> {
        match group.family {
            FAMILY_ELLIPSOID => Some(&self.ellipsoid),
            FAMILY_BOX => Some(&self.oriented_box),
            FAMILY_POLYGON => Some(&self.polygon),
            FAMILY_PARTICLE => self
                .particle
                .get(group.shape as usize)
                .and_then(Option::as_ref),
            _ => None,
        }
    }
}
