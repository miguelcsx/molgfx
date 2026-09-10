//! Pipeline construction for the transparency pass.
//!
//! Every specialization the transparent geometry needs is built here at load,
//! never inside the frame loop. Splitting a shader into one pipeline per
//! variant is what keeps a clip test or a grid march off the fragments that
//! do not need it.

use crate::error::RenderError;
use crate::passes::primitive_pipelines::{PrimitivePipelineSet, TRANSPARENT_ENTRIES};
use crate::passes::segmentation_targets;
use crate::passes::visual_pipelines::{VisualPipelineSet, constants};
use crate::scene_gpu::{
    GENERIC_INSTANCE_CAPSULE, GENERIC_INSTANCE_SPHERE, SegmentationPipelineKey,
};
use pdviewx_core::VolumeRendering;
use pdviewx_gpu::{
    BlendMode, ColorTarget, CompareFunction, DepthState, Device, PrimitiveTopology,
    RenderPipelineDesc, ShaderModuleDesc, TextureFormat,
};

#[cfg(test)]
#[path = "pipelines_tests.rs"]
mod tests;

const VOLUME_MODE_CONSTANT: &str = "VOLUME_RENDER_MODE";
const SEGMENTATION_SLICE_CONSTANT: &str = "SEGMENTATION_SLICE_MODE";
const SEGMENTATION_HASH_CONSTANT: &str = "SEGMENTATION_HASH_LOOKUP";

#[derive(Debug)]
pub(super) struct VolumePipelineSet<D: Device> {
    direct: D::Pipeline,
    isosurface: D::Pipeline,
    medium: D::Pipeline,
    slice: D::Pipeline,
    liquid: D::Pipeline,
}

impl<D: Device> VolumePipelineSet<D> {
    pub(super) const fn get(&self, rendering: VolumeRendering) -> &D::Pipeline {
        match rendering {
            VolumeRendering::Direct => &self.direct,
            VolumeRendering::Isosurface => &self.isosurface,
            VolumeRendering::Medium => &self.medium,
            VolumeRendering::Slice => &self.slice,
            VolumeRendering::LiquidSurface => &self.liquid,
        }
    }
}

#[derive(Debug)]
pub(super) struct SegmentationPipelineSet<D: Device> {
    direct: D::Pipeline,
    direct_slice: D::Pipeline,
    hash: D::Pipeline,
    hash_slice: D::Pipeline,
}

impl<D: Device> SegmentationPipelineSet<D> {
    pub(super) const fn get(&self, key: SegmentationPipelineKey) -> &D::Pipeline {
        match key {
            SegmentationPipelineKey::Direct => &self.direct,
            SegmentationPipelineKey::DirectSlice => &self.direct_slice,
            SegmentationPipelineKey::Hash => &self.hash,
            SegmentationPipelineKey::HashSlice => &self.hash_slice,
        }
    }
}

/// The order-independent transparency accumulation and revealage targets, and
/// the read-only depth every transparent pass shares.
pub(super) fn oit_targets() -> [ColorTarget; 2] {
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

pub(super) fn oit_depth() -> DepthState {
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
) -> Result<(VisualPipelineSet<D>, VisualPipelineSet<D>), RenderError> {
    let build = |label, fragment| {
        visual_pipeline(
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
) -> Result<(VisualPipelineSet<D>, VisualPipelineSet<D>), RenderError> {
    let build = |label, fragment| {
        visual_pipeline(
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

pub(super) fn visual_pipeline<D: Device>(
    device: &D,
    group0: &D::BindGroupLayout,
    group1: &D::BindGroupLayout,
    desc: &OitPipelineDesc<'_, D>,
) -> Result<VisualPipelineSet<D>, RenderError> {
    let shader = device.create_shader_module(&ShaderModuleDesc {
        label: desc.label,
        wgsl: desc.wgsl,
    })?;
    let build = |pipeline_constants: &[(&'static str, f64)]| {
        device.create_render_pipeline(&RenderPipelineDesc {
            label: desc.label,
            layouts: &[Some(group0), Some(group1), Some(desc.group2)],
            shader: &shader,
            vs_entry: desc.vertex,
            fs_entry: Some(desc.fragment),
            color_targets: &oit_targets(),
            depth: Some(oit_depth()),
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

pub(super) fn generic_instance_pipelines<D: Device>(
    device: &D,
    group0: &D::BindGroupLayout,
    group1: &D::BindGroupLayout,
    group2: &D::BindGroupLayout,
) -> Result<(VisualPipelineSet<D>, VisualPipelineSet<D>), RenderError> {
    let shader = device.create_shader_module(&ShaderModuleDesc {
        label: "transparent generic analytic instances",
        wgsl: pdviewx_shaders::GENERIC_INSTANCE,
    })?;
    let build = |label, shape, visual, fragment| {
        device.create_render_pipeline(&RenderPipelineDesc {
            label,
            layouts: &[Some(group0), Some(group1), Some(group2)],
            shader: &shader,
            vs_entry: "vs_generic_instance",
            fs_entry: Some("fs_generic_instance_transparent"),
            color_targets: &oit_targets(),
            depth: Some(oit_depth()),
            constants: &[
                ("GENERIC_INSTANCE_SHAPE", f64::from(shape)),
                ("PARTICLE_SHAPE_KIND", f64::from(shape)),
                ("VISUAL_PROGRAM_ENABLED", if visual { 1.0 } else { 0.0 }),
                ("VISUAL_FRAGMENT_ENABLED", if fragment { 1.0 } else { 0.0 }),
            ],
            topology: PrimitiveTopology::TriangleList,
        })
    };
    let set = |label, shape| -> Result<VisualPipelineSet<D>, RenderError> {
        Ok(VisualPipelineSet::new(
            build(label, shape, false, false)?,
            build(label, shape, true, false)?,
            build(label, shape, true, true)?,
        ))
    };
    Ok((
        set(
            "transparent generic analytic instance spheres",
            GENERIC_INSTANCE_SPHERE,
        )?,
        set(
            "transparent generic analytic instance capsules",
            GENERIC_INSTANCE_CAPSULE,
        )?,
    ))
}

pub(super) fn volume_pipelines<D: Device>(
    device: &D,
    group0: &D::BindGroupLayout,
    group1: &D::BindGroupLayout,
    group2: &D::BindGroupLayout,
) -> Result<VolumePipelineSet<D>, RenderError> {
    let shader = device.create_shader_module(&ShaderModuleDesc {
        label: "density volumes",
        wgsl: pdviewx_shaders::VOLUME,
    })?;
    let build = |label, rendering| {
        let constants = volume_pipeline_constants(rendering);
        device.create_render_pipeline(&RenderPipelineDesc {
            label,
            layouts: &[Some(group0), Some(group1), Some(group2)],
            shader: &shader,
            vs_entry: "vs_volume",
            fs_entry: Some("fs_volume"),
            color_targets: &oit_targets(),
            depth: Some(oit_depth()),
            constants: &constants,
            topology: PrimitiveTopology::TriangleList,
        })
    };
    Ok(VolumePipelineSet {
        direct: build("direct density volumes", VolumeRendering::Direct)?,
        isosurface: build("density isosurfaces", VolumeRendering::Isosurface)?,
        medium: build("participating density media", VolumeRendering::Medium)?,
        slice: build("density volume slices", VolumeRendering::Slice)?,
        liquid: build("liquid density surfaces", VolumeRendering::LiquidSurface)?,
    })
}

pub(super) fn segmentation_pipelines<D: Device>(
    device: &D,
    group0: &D::BindGroupLayout,
    group1: &D::BindGroupLayout,
    group2: &D::BindGroupLayout,
) -> Result<SegmentationPipelineSet<D>, RenderError> {
    let shader = device.create_shader_module(&ShaderModuleDesc {
        label: "categorical segmentation volumes",
        wgsl: pdviewx_shaders::SEGMENTATION,
    })?;
    let color_targets = segmentation_targets();
    let build = |label, key| {
        let constants = segmentation_pipeline_constants(key);
        device.create_render_pipeline(&RenderPipelineDesc {
            label,
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
            constants: &constants,
            topology: PrimitiveTopology::TriangleList,
        })
    };
    Ok(SegmentationPipelineSet {
        direct: build(
            "categorical segmentation volumes",
            SegmentationPipelineKey::Direct,
        )?,
        direct_slice: build(
            "categorical segmentation slices",
            SegmentationPipelineKey::DirectSlice,
        )?,
        hash: build(
            "sparse categorical segmentation volumes",
            SegmentationPipelineKey::Hash,
        )?,
        hash_slice: build(
            "sparse categorical segmentation slices",
            SegmentationPipelineKey::HashSlice,
        )?,
    })
}

const fn volume_pipeline_constants(rendering: VolumeRendering) -> [(&'static str, f64); 1] {
    let value = match rendering {
        VolumeRendering::Direct => 0.0,
        VolumeRendering::Isosurface => 1.0,
        VolumeRendering::Medium => 2.0,
        VolumeRendering::Slice => 3.0,
        VolumeRendering::LiquidSurface => 4.0,
    };
    [(VOLUME_MODE_CONSTANT, value)]
}

const fn segmentation_pipeline_constants(key: SegmentationPipelineKey) -> [(&'static str, f64); 2] {
    let slice = if key.is_slice() { 1.0 } else { 0.0 };
    let hash = if key.is_hash() { 1.0 } else { 0.0 };
    [
        (SEGMENTATION_SLICE_CONSTANT, slice),
        (SEGMENTATION_HASH_CONSTANT, hash),
    ]
}
