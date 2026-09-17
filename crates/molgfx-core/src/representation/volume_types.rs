//! Fixed-capacity volume presentation data.

use crate::{ClipPlane, CoreError};
use molgfx_math::Rgba8;

/// Maximum transfer points kept in one compact volume uniform.
pub const MAX_VOLUME_TRANSFER_POINTS: usize = 8;

/// One scalar-to-color-and-opacity transfer control point.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct VolumeTransferPoint {
    /// Scalar value at this point.
    pub value: f32,
    /// Reversible scientific color.
    pub color: Rgba8,
    /// Optical response in [0, 1].
    pub opacity: f32,
}

impl VolumeTransferPoint {
    /// Creates one point.
    #[must_use]
    pub const fn new(value: f32, color: Rgba8, opacity: f32) -> Self {
        Self {
            value,
            color,
            opacity,
        }
    }
}

/// Ordered, fixed-capacity direct-volume transfer function.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct VolumeTransferFunction {
    points: [VolumeTransferPoint; MAX_VOLUME_TRANSFER_POINTS],
    len: u8,
}

impl VolumeTransferFunction {
    /// Validates two to eight strictly ordered finite control points.
    ///
    /// # Errors
    ///
    /// Values must increase strictly and opacities must be finite in [0, 1].
    pub fn new(points: &[VolumeTransferPoint]) -> Result<Self, CoreError> {
        if !(2..=MAX_VOLUME_TRANSFER_POINTS).contains(&points.len()) {
            return Err(invalid_transfer(
                "transfer function requires two to eight points",
            ));
        }
        if points.iter().any(|point| {
            !point.value.is_finite()
                || !point.opacity.is_finite()
                || !(0.0..=1.0).contains(&point.opacity)
        }) || points.windows(2).any(|pair| pair[0].value >= pair[1].value)
        {
            return Err(invalid_transfer(
                "transfer values must increase and opacities must be finite",
            ));
        }
        let mut transfer = Self::default();
        transfer.points[..points.len()].copy_from_slice(points);
        transfer.len = u8::try_from(points.len()).map_or(2, |len| len);
        Ok(transfer)
    }

    /// Linear transparent-to-opaque map over one scalar interval.
    #[must_use]
    pub fn linear(range: [f32; 2], low: Rgba8, high: Rgba8) -> Self {
        let high_value = if range[1].is_finite() && range[1] > range[0] {
            range[1]
        } else {
            range[0] + 1.0
        };
        Self {
            points: [
                VolumeTransferPoint::new(range[0], low, 0.0),
                VolumeTransferPoint::new(high_value, high, 1.0),
                VolumeTransferPoint::new(0.0, Rgba8::WHITE, 0.0),
                VolumeTransferPoint::new(0.0, Rgba8::WHITE, 0.0),
                VolumeTransferPoint::new(0.0, Rgba8::WHITE, 0.0),
                VolumeTransferPoint::new(0.0, Rgba8::WHITE, 0.0),
                VolumeTransferPoint::new(0.0, Rgba8::WHITE, 0.0),
                VolumeTransferPoint::new(0.0, Rgba8::WHITE, 0.0),
            ],
            len: 2,
        }
    }

    /// Active points in scalar order.
    #[must_use]
    pub fn points(&self) -> &[VolumeTransferPoint] {
        &self.points[..usize::from(self.len)]
    }
}

impl Default for VolumeTransferFunction {
    fn default() -> Self {
        Self::linear(
            [0.0, 1.0],
            Rgba8::opaque(68, 1, 84),
            Rgba8::opaque(253, 231, 37),
        )
    }
}

/// Transfer function and sampling controls for a density volume.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct VolumeStyle {
    /// Direct integration or one lit scalar isosurface over the same grid.
    pub rendering: VolumeRendering,
    /// Piecewise-linear scalar, colour and opacity mapping.
    pub transfer: VolumeTransferFunction,
    /// Optical-density multiplier applied after material opacity.
    pub opacity_scale: f32,
    /// Ray step relative to the smallest voxel axis; lower is more accurate.
    pub step_scale: f32,
    /// World-space plane sampled by [`VolumeRendering::Slice`].
    pub slice: Option<VolumeSlice>,
    /// Optional half-open voxel region rendered from the resident grid.
    pub region: Option<VolumeRegion>,
}

impl Default for VolumeStyle {
    fn default() -> Self {
        Self {
            rendering: VolumeRendering::Direct,
            transfer: VolumeTransferFunction::default(),
            opacity_scale: 2.0,
            step_scale: 0.65,
            slice: None,
            region: None,
        }
    }
}

/// A validated half-open voxel region `[minimum, maximum)`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct VolumeRegion {
    minimum: [u32; 3],
    maximum: [u32; 3],
}

impl VolumeRegion {
    /// Validates a crop against its source grid dimensions.
    ///
    /// # Errors
    ///
    /// Every axis must be non-empty and remain inside `dimensions`.
    pub fn new(
        minimum: [u32; 3],
        maximum: [u32; 3],
        dimensions: [u32; 3],
    ) -> Result<Self, CoreError> {
        if (0..3).any(|axis| minimum[axis] >= maximum[axis] || maximum[axis] > dimensions[axis]) {
            return Err(invalid_transfer(
                "volume region must be non-empty and inside the source grid",
            ));
        }
        Ok(Self { minimum, maximum })
    }

    /// Inclusive minimum voxel index.
    #[must_use]
    pub const fn minimum(self) -> [u32; 3] {
        self.minimum
    }

    /// Exclusive maximum voxel index.
    #[must_use]
    pub const fn maximum(self) -> [u32; 3] {
        self.maximum
    }
}

/// One arbitrary world-space plane through a caller scalar grid.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct VolumeSlice {
    /// Plane equation; unlike clipping, this is the surface that is drawn.
    pub plane: ClipPlane,
}

impl VolumeSlice {
    /// Creates a slice from a validated world-space plane.
    #[must_use]
    pub const fn new(plane: ClipPlane) -> Self {
        Self { plane }
    }
}

/// Rendering algorithm over one caller-supplied scalar grid.
#[repr(u32)]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum VolumeRendering {
    /// Front-to-back optical integration through the scalar field.
    #[default]
    Direct = 0,
    /// Lit implicit boundary at the representation isolevel.
    Isosurface = 1,
    /// Single-scattered participating medium from caller-supplied density.
    Medium = 2,
    /// Transfer-mapped scalar values on one arbitrary world-space plane.
    Slice = 3,
    /// Screen-space liquid-like boundary over caller-provided scalar density.
    /// No advection or fluid simulation is performed by the renderer.
    LiquidSurface = 4,
}

const fn invalid_transfer(reason: &'static str) -> CoreError {
    CoreError::InvalidVolume { reason }
}
