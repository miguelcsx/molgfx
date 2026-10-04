//! Portable readings of one scalar grid.

use crate::{Color, Error};
use molgfx_core::{RepresentationConfig, VolumeRendering, VolumeStyle, VolumeTransferFunction};
use molgfx_math::Vec3;
use serde::{Deserialize, Serialize};

#[cfg(test)]
#[path = "volume_presentation_tests.rs"]
mod tests;

/// Appearance of an implicit scalar boundary, evaluated in voxel coordinates.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum IsoStyle {
    /// Continuous lit boundary.
    Solid,
    /// Lines near integer voxel planes on the two tangent axes.
    Mesh {
        /// Half-width of a lattice line in voxels.
        line_width_voxels: f32,
    },
    /// Spots near the two-dimensional tangent voxel lattice.
    Dots {
        /// Radius of a lattice spot in voxels.
        dot_radius_voxels: f32,
    },
}

/// One piecewise-linear scalar, color and opacity control point.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub struct VolumeTransferPoint {
    /// Scalar coordinate.
    pub value: f32,
    /// Physical color.
    pub color: Color,
    /// Optical response in zero to one.
    pub opacity: f32,
}

/// Independent presentation sharing one scalar-grid upload.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum VolumePresentation {
    /// Lit scalar boundary.
    Isosurface {
        /// Absolute scalar level.
        isovalue: f32,
        /// Boundary color.
        color: Color,
        /// Boundary opacity.
        opacity: f32,
        /// Lattice appearance.
        style: IsoStyle,
    },
    /// Front-to-back optical integration.
    Direct {
        /// Two to eight strictly increasing control points.
        transfer: Vec<VolumeTransferPoint>,
        /// Non-negative optical-density multiplier.
        opacity_scale: f32,
        /// Positive relative sample spacing.
        step_scale: f32,
    },
    /// One arbitrary world-space plane colored through an explicit scalar domain.
    Slice {
        /// Point on the slice plane in world ångström.
        point: [f32; 3],
        /// Non-zero plane normal.
        normal: [f32; 3],
        /// Named color ramp.
        ramp: Box<str>,
        /// Finite increasing scalar domain.
        domain: [f32; 2],
    },
    /// Single-scattered participating density medium.
    Medium {
        /// Scalar transfer controls.
        transfer: Vec<VolumeTransferPoint>,
        /// Optical-density multiplier.
        opacity_scale: f32,
        /// Relative sample spacing.
        step_scale: f32,
    },
    /// Lit liquid-like implicit boundary; no fluid simulation is implied.
    LiquidSurface {
        /// Absolute scalar level.
        isovalue: f32,
        /// Boundary color.
        color: Color,
        /// Boundary opacity.
        opacity: f32,
    },
}

impl VolumePresentation {
    pub(crate) fn native(&self) -> Result<RepresentationConfig, Error> {
        let config = molgfx_core::Representation::volume();
        Ok(match self {
            Self::Isosurface {
                isovalue,
                color,
                opacity,
                style,
            } => {
                let (rendering, width) = match style {
                    IsoStyle::Solid => (VolumeRendering::Isosurface, 0.0),
                    IsoStyle::Mesh { line_width_voxels } => {
                        (VolumeRendering::IsoMesh, lattice_width(*line_width_voxels)?)
                    }
                    IsoStyle::Dots { dot_radius_voxels } => {
                        (VolumeRendering::IsoDots, lattice_width(*dot_radius_voxels)?)
                    }
                };
                boundary(
                    config,
                    *isovalue,
                    *color,
                    *opacity,
                    &VolumeStyle {
                        rendering,
                        iso_width_voxels: width,
                        ..VolumeStyle::isosurface()
                    },
                )?
            }
            Self::LiquidSurface {
                isovalue,
                color,
                opacity,
            } => boundary(
                config,
                *isovalue,
                *color,
                *opacity,
                &VolumeStyle::liquid_surface(),
            )?,
            Self::Direct {
                transfer,
                opacity_scale,
                step_scale,
            }
            | Self::Medium {
                transfer,
                opacity_scale,
                step_scale,
            } => {
                if !opacity_scale.is_finite()
                    || *opacity_scale < 0.0
                    || !step_scale.is_finite()
                    || *step_scale <= 0.0
                {
                    return Err(invalid(
                        "volume sampling requires finite non-negative density and positive step spacing",
                    ));
                }
                let style = if matches!(self, Self::Medium { .. }) {
                    VolumeStyle::medium()
                } else {
                    VolumeStyle::default()
                };
                config.volume_style(
                    style
                        .transfer(native_transfer(transfer)?)
                        .sampling(*opacity_scale, *step_scale),
                )
            }
            Self::Slice {
                point,
                normal,
                ramp,
                domain,
            } => slice(config, *point, *normal, ramp, *domain)?,
        })
    }
}

fn slice(
    config: RepresentationConfig,
    point: [f32; 3],
    normal: [f32; 3],
    ramp: &str,
    domain: [f32; 2],
) -> Result<RepresentationConfig, Error> {
    if !domain.iter().all(|value| value.is_finite()) || domain[0] >= domain[1] {
        return Err(invalid("slice domain must be finite and increasing"));
    }
    let plane = molgfx_core::ClipPlane::from_point_normal(
        Vec3::from_array(point),
        Vec3::from_array(normal),
    )?;
    let colors = crate::color::ramp_colors(ramp)?;
    let native_colors: Vec<_> = colors.iter().map(|color| color.native()).collect();
    let ramp = molgfx_core::ScalarRamp::evenly(domain, &native_colors)?;
    Ok(config.volume_style(
        VolumeStyle::slice(molgfx_core::VolumeSlice::new(plane))
            .slice_ramp(ramp)
            .sampling(1.0, 0.65),
    ))
}

fn boundary(
    config: RepresentationConfig,
    level: f32,
    color: Color,
    opacity: f32,
    style: &VolumeStyle,
) -> Result<RepresentationConfig, Error> {
    if !level.is_finite() || !opacity.is_finite() || !(0.0..=1.0).contains(&opacity) {
        return Err(invalid(
            "volume boundary level must be finite and opacity within zero to one",
        ));
    }
    let transfer = VolumeTransferFunction::new(&[
        molgfx_core::VolumeTransferPoint::new(0.0, color.native(), 1.0),
        molgfx_core::VolumeTransferPoint::new(1.0, color.native(), 1.0),
    ])?;
    Ok(config
        .isolevel(level)
        .material(molgfx_core::Material {
            opacity,
            ..molgfx_core::Material::default()
        })
        .volume_style((*style).transfer(transfer)))
}

fn native_transfer(points: &[VolumeTransferPoint]) -> Result<VolumeTransferFunction, Error> {
    if !(2..=8).contains(&points.len()) {
        return Err(invalid("volume transfer requires two to eight points"));
    }
    let mut native =
        [molgfx_core::VolumeTransferPoint::new(0.0, molgfx_math::Rgba8::WHITE, 0.0); 8];
    for (out, point) in native.iter_mut().zip(points) {
        *out =
            molgfx_core::VolumeTransferPoint::new(point.value, point.color.native(), point.opacity);
    }
    native_transfer_points(&native[..points.len()])
}

fn native_transfer_points(
    points: &[molgfx_core::VolumeTransferPoint],
) -> Result<VolumeTransferFunction, Error> {
    Ok(VolumeTransferFunction::new(points)?)
}
fn lattice_width(value: f32) -> Result<f32, Error> {
    if !value.is_finite() || value <= 0.0 || value > 0.5 {
        return Err(invalid("voxel lattice width must lie in (0, 0.5]"));
    }
    Ok(value)
}
fn invalid(message: &str) -> Error {
    Error::InvalidSpec(message.to_owned())
}
