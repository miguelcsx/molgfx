//! Pipeline construction for the transparency pass.
//!
//! Every specialization the transparent geometry needs is built here at load,
//! never inside the frame loop. Splitting a shader into one pipeline per
//! variant is what keeps a clip test or a grid march off the fragments that
//! do not need it.

use crate::error::RenderError;
use crate::passes::primitive_pipelines::{PrimitivePipelineSet, TRANSPARENT_ENTRIES};
use crate::passes::segmentation_targets;
use pdviewx_gpu::{
    BlendMode, ColorTarget, CompareFunction, DepthState, Device, PrimitiveTopology,
    RenderPipelineDesc, ShaderModuleDesc, TextureFormat,
};

/// The order-independent transparency accumulation and revealage targets, and
/// the read-only depth every transparent pass shares.
fn oit_targets() -> [ColorTarget; 2] {
    [
        ColorTarget {
            format: TextureFormat::Rgba16Float,
            blend: BlendMode::Additive,
        },
        ColorTarget {
            format: TextureFormat::R8Unorm,
            blend: BlendMode::ReverseMultiply,
        },
    ]
}

fn oit_depth() -> DepthState {
    DepthState {
        format: TextureFormat::Depth32Float,
        write: false,
        compare: CompareFunction::GreaterEqual,
    }
}

pub(super) struct OitPipelineDesc<'a, D: Device> {
    pub(super) label: &'static str,
    pub(super) wgsl: &'static str,
    pub(super) vertex: &'static str,
    pub(super) fragment: &'static str,
    pub(super) group2: &'a D::BindGroupLayout,
}

/// The transparent primitive class set: one pipeline per family and per
/// particle shape, sharing the transparency targets. Group 1 is the pass
/// input; the primitive vertex stage does not read it, but the layout must
/// still match the pipeline layout the pass binds.
pub(super) fn primitive_pipelines<D: Device>(
    device: &D,
    group0: &D::BindGroupLayout,
    group1: &D::BindGroupLayout,
    group2: &D::BindGroupLayout,
) -> Result<PrimitivePipelineSet<D>, RenderError> {
    PrimitivePipelineSet::build(
        device,
        group0,
        Some(group1),
        group2,
        &TRANSPARENT_ENTRIES,
        &oit_targets(),
        Some(oit_depth()),
    )
}

/// The transparent sphere pair: clipped and unclipped fragment paths.
///
/// Separate pipelines keep the clip test off representations that declare no
/// clip planes, which is every representation in the common case.
pub(super) fn sphere_pipelines<D: Device>(
    device: &D,
    group0: &D::BindGroupLayout,
    group1: &D::BindGroupLayout,
    group2: &D::BindGroupLayout,
) -> Result<(D::Pipeline, D::Pipeline), RenderError> {
    let build = |label, fragment| {
        pipeline(
            device,
            group0,
            group1,
            &OitPipelineDesc {
                label,
                wgsl: pdviewx_shaders::GEOMETRY_SPHERE,
                vertex: "vs_sphere_transparent",
                fragment,
                group2,
            },
        )
    };
    Ok((
        build("transparent sphere impostors", "fs_sphere_transparent")?,
        build(
            "clipped transparent sphere impostors",
            "fs_sphere_transparent_clipped",
        )?,
    ))
}

/// The transparent surface pair: analytic union and persistent-grid tracing.
pub(super) fn surface_pipelines<D: Device>(
    device: &D,
    group0: &D::BindGroupLayout,
    group1: &D::BindGroupLayout,
    group2: &D::BindGroupLayout,
) -> Result<(D::Pipeline, D::Pipeline), RenderError> {
    let build = |label, fragment| {
        pipeline(
            device,
            group0,
            group1,
            &OitPipelineDesc {
                label,
                wgsl: pdviewx_shaders::GEOMETRY_SURFACE,
                vertex: "vs_surface",
                fragment,
                group2,
            },
        )
    };
    Ok((
        build(
            "transparent analytic molecular surfaces",
            "fs_surface_union_transparent",
        )?,
        build(
            "transparent field molecular surfaces",
            "fs_surface_grid_transparent",
        )?,
    ))
}

pub(super) fn pipeline<D: Device>(
    device: &D,
    group0: &D::BindGroupLayout,
    group1: &D::BindGroupLayout,
    desc: &OitPipelineDesc<'_, D>,
) -> Result<D::Pipeline, RenderError> {
    let shader = device.create_shader_module(&ShaderModuleDesc {
        label: desc.label,
        wgsl: desc.wgsl,
    })?;
    Ok(device.create_render_pipeline(&RenderPipelineDesc {
        label: desc.label,
        layouts: &[Some(group0), Some(group1), Some(desc.group2)],
        shader: &shader,
        vs_entry: desc.vertex,
        fs_entry: Some(desc.fragment),
        color_targets: &oit_targets(),
        depth: Some(oit_depth()),
        constants: &[],
        topology: PrimitiveTopology::TriangleList,
    })?)
}

pub(super) fn segmentation_pipeline<D: Device>(
    device: &D,
    group0: &D::BindGroupLayout,
    group1: &D::BindGroupLayout,
    group2: &D::BindGroupLayout,
) -> Result<D::Pipeline, RenderError> {
    let shader = device.create_shader_module(&ShaderModuleDesc {
        label: "categorical segmentation volumes",
        wgsl: pdviewx_shaders::SEGMENTATION,
    })?;
    let color_targets = segmentation_targets();
    Ok(device.create_render_pipeline(&RenderPipelineDesc {
        label: "categorical segmentation volumes",
        layouts: &[Some(group0), Some(group1), Some(group2)],
        shader: &shader,
        vs_entry: "vs_segmentation",
        fs_entry: Some("fs_segmentation"),
        color_targets: &color_targets,
        depth: Some(DepthState {
            format: TextureFormat::Depth32Float,
            write: false,
            compare: CompareFunction::GreaterEqual,
        }),
        constants: &[],
        topology: PrimitiveTopology::TriangleList,
    })?)
}
