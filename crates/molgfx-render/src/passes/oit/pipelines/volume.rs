//! Scalar pipeline specialization shares one shader across source geometry kinds.
use super::{oit_depth, oit_targets};
use crate::{RenderError, scene_gpu::GeometryKind};
use molgfx_core::VolumeRendering;
use molgfx_gpu::{Device, PrimitiveTopology, RenderPipelineDesc, ShaderModuleDesc};
const VOLUME_MODE_CONSTANT: &str = "VOLUME_RENDER_MODE";

#[derive(Debug)]
pub(in crate::passes::oit) struct VolumePipelineSet<D: Device> {
    direct: D::Pipeline,
    isosurface: D::Pipeline,
    medium: D::Pipeline,
    slice: D::Pipeline,
    liquid: D::Pipeline,
    mesh: D::Pipeline,
    dots: D::Pipeline,
    boundary_mesh: D::Pipeline,
    boundary_dots: D::Pipeline,
}

impl<D: Device> VolumePipelineSet<D> {
    pub(in crate::passes::oit) const fn get(
        &self,
        rendering: VolumeRendering,
        geometry: GeometryKind,
    ) -> &D::Pipeline {
        match (rendering, geometry) {
            (VolumeRendering::IsoMesh, GeometryKind::Boundary) => &self.boundary_mesh,
            (VolumeRendering::IsoDots, GeometryKind::Boundary) => &self.boundary_dots,
            (VolumeRendering::Direct, _) => &self.direct,
            (VolumeRendering::Isosurface, _) => &self.isosurface,
            (VolumeRendering::Medium, _) => &self.medium,
            (VolumeRendering::Slice, _) => &self.slice,
            (VolumeRendering::LiquidSurface, _) => &self.liquid,
            (VolumeRendering::IsoMesh, _) => &self.mesh,
            (VolumeRendering::IsoDots, _) => &self.dots,
        }
    }
}

pub(in crate::passes::oit) fn volume_pipelines<D: Device>(
    device: &D,
    group0: &D::BindGroupLayout,
    group1: &D::BindGroupLayout,
    group2: &D::BindGroupLayout,
    group3: &D::BindGroupLayout,
) -> Result<VolumePipelineSet<D>, RenderError> {
    let shader = device.create_shader_module(&ShaderModuleDesc {
        label: "density volumes",
        wgsl: molgfx_shaders::VOLUME,
    })?;
    let build = |label, rendering, geometry| {
        let constants = volume_pipeline_constants(rendering);
        device.create_render_pipeline(&RenderPipelineDesc {
            label,
            layouts: &[
                Some(group0),
                Some(group1),
                Some(group2),
                (geometry == GeometryKind::Boundary).then_some(group3),
            ],
            shader: &shader,
            vs_entry: if geometry == GeometryKind::Boundary {
                "vs_volume_boundary"
            } else {
                "vs_volume"
            },
            fs_entry: Some(if geometry == GeometryKind::Boundary {
                "fs_volume_boundary"
            } else {
                "fs_volume"
            }),
            color_targets: &oit_targets(),
            depth: Some(oit_depth()),
            constants: &constants,
            topology: PrimitiveTopology::TriangleList,
        })
    };
    Ok(VolumePipelineSet {
        direct: build(
            "direct density volumes",
            VolumeRendering::Direct,
            GeometryKind::Proxy,
        )?,
        isosurface: build(
            "density isosurfaces",
            VolumeRendering::Isosurface,
            GeometryKind::Proxy,
        )?,
        medium: build(
            "participating density media",
            VolumeRendering::Medium,
            GeometryKind::Proxy,
        )?,
        slice: build(
            "density volume slices",
            VolumeRendering::Slice,
            GeometryKind::Proxy,
        )?,
        liquid: build(
            "liquid density surfaces",
            VolumeRendering::LiquidSurface,
            GeometryKind::Proxy,
        )?,
        mesh: build(
            "density isosurface lattice lines",
            VolumeRendering::IsoMesh,
            GeometryKind::Proxy,
        )?,
        dots: build(
            "density isosurface lattice dots",
            VolumeRendering::IsoDots,
            GeometryKind::Proxy,
        )?,
        boundary_mesh: build(
            "indexed scalar lattice lines",
            VolumeRendering::IsoMesh,
            GeometryKind::Boundary,
        )?,
        boundary_dots: build(
            "indexed scalar lattice dots",
            VolumeRendering::IsoDots,
            GeometryKind::Boundary,
        )?,
    })
}

pub(super) const fn volume_pipeline_constants(
    rendering: VolumeRendering,
) -> [(&'static str, f64); 1] {
    let value = match rendering {
        VolumeRendering::Direct => 0.0,
        VolumeRendering::Isosurface => 1.0,
        VolumeRendering::Medium => 2.0,
        VolumeRendering::Slice => 3.0,
        VolumeRendering::LiquidSurface => 4.0,
        VolumeRendering::IsoMesh => 5.0,
        VolumeRendering::IsoDots => 6.0,
    };
    [(VOLUME_MODE_CONSTANT, value)]
}
