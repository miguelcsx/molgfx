//! Reconstruction of scalar-volume and categorical-volume presentation.

use super::super::color::rgba;
use super::super::invalid;
use crate::serialization::types::{self, SegmentationStyleDescription, VolumeStyleDescription};
use crate::{
    ClipPlane, ScalarRamp, SegmentStyle, SegmentStyleTable, VolumeRegion, VolumeRendering,
    VolumeSlice, VolumeStyle, VolumeTransferFunction, VolumeTransferPoint,
};
use molgfx_math::Vec3;

pub(super) fn parse_volume_style(
    value: &VolumeStyleDescription,
) -> Result<VolumeStyle, crate::CoreError> {
    let points = value
        .transfer
        .iter()
        .map(|point| VolumeTransferPoint::new(point.value, rgba(point.color), point.opacity))
        .collect::<Vec<_>>();
    let transfer = VolumeTransferFunction::new(&points)?;
    let style = VolumeStyle {
        rendering: parse_volume_rendering(&value.rendering)?,
        transfer,
        opacity_scale: finite_nonnegative(value.opacity_scale, "volume opacity scale")?,
        step_scale: positive_finite(value.step_scale, "volume step scale")?,
        slice: value.slice.map(parse_slice).transpose()?,
        slice_ramp: value
            .slice_ramp
            .as_deref()
            .map(parse_slice_ramp)
            .transpose()?,
        region: value.region.map(parse_region).transpose()?,
        iso_width_voxels: finite_nonnegative(value.iso_width_voxels, "volume lattice width")?,
    };
    style.validate()?;
    Ok(style)
}

fn parse_slice_ramp(
    points: &[types::VolumeTransferPointDescription],
) -> Result<ScalarRamp, crate::CoreError> {
    if points
        .iter()
        .any(|point| point.opacity.to_bits() != 1.0_f32.to_bits())
    {
        return invalid("slice palette stops must have opacity one");
    }
    let values: Vec<_> = points.iter().map(|point| point.value).collect();
    let colors: Vec<_> = points.iter().map(|point| rgba(point.color)).collect();
    ScalarRamp::new(&values, &colors)
}

fn parse_volume_rendering(value: &str) -> Result<VolumeRendering, crate::CoreError> {
    match value {
        "direct" => Ok(VolumeRendering::Direct),
        "isosurface" => Ok(VolumeRendering::Isosurface),
        "medium" => Ok(VolumeRendering::Medium),
        "slice" => Ok(VolumeRendering::Slice),
        "liquid_surface" => Ok(VolumeRendering::LiquidSurface),
        "iso_mesh" => Ok(VolumeRendering::IsoMesh),
        "iso_dots" => Ok(VolumeRendering::IsoDots),
        _ => invalid("unknown volume rendering mode"),
    }
}

pub(super) fn parse_segmentation_style(
    value: &SegmentationStyleDescription,
) -> Result<crate::SegmentationStyle, crate::CoreError> {
    let styles = value
        .styles
        .iter()
        .map(|style| SegmentStyle::new(style.label, rgba(style.color), style.opacity))
        .collect::<Vec<_>>();
    Ok(crate::SegmentationStyle {
        presentation: value.presentation,
        styles: SegmentStyleTable::new(&styles)?,
        opacity_scale: finite_nonnegative(value.opacity_scale, "segmentation opacity scale")?,
        slice: value.slice.map(parse_slice).transpose()?,
        region: value.region.map(parse_region).transpose()?,
    })
}

fn parse_slice(value: [f32; 4]) -> Result<VolumeSlice, crate::CoreError> {
    let normal = Vec3::new(value[0], value[1], value[2]);
    if !normal.is_finite() || !value[3].is_finite() || normal.length_squared() <= 1.0e-12 {
        return invalid("volume slice plane is malformed");
    }
    Ok(VolumeSlice::new(ClipPlane {
        normal,
        offset: value[3],
    }))
}

fn parse_region(value: types::RegionDescription) -> Result<VolumeRegion, crate::CoreError> {
    if (0..3).any(|axis| value.minimum[axis] >= value.maximum[axis]) {
        return invalid("volume region is empty");
    }
    VolumeRegion::new(value.minimum, value.maximum, [u32::MAX; 3])
}

fn finite_nonnegative(value: f32, label: &'static str) -> Result<f32, crate::CoreError> {
    if value.is_finite() && value >= 0.0 {
        Ok(value)
    } else {
        invalid(label)
    }
}

fn positive_finite(value: f32, label: &'static str) -> Result<f32, crate::CoreError> {
    if value.is_finite() && value > 0.0 {
        Ok(value)
    } else {
        invalid(label)
    }
}
