//! Camera-space optical presentation settings.

use super::profile_numeric::{finite_clamp, lerp, unit};
use molgfx_core::SelectionHandle;
use molgfx_math::Vec3;
use serde::{Deserialize, Serialize};

/// A physical or scene-tracked focal plane for thin-lens presentation.
#[derive(Clone, Copy, PartialEq, Debug, Default, Serialize, Deserialize)]
pub enum FocusTarget {
    /// Follow the camera's look-at target.
    #[default]
    CameraTarget,
    /// Use an explicit positive view-space distance in Ångström.
    Distance(f32),
    /// Track one world-space point as the camera moves.
    WorldPoint(Vec3),
    /// Track the centroid of a caller-authored molecular selection.
    Selection(SelectionHandle),
}

impl FocusTarget {
    pub(super) fn sanitize(self) -> Self {
        match self {
            Self::Distance(distance) if distance.is_finite() && distance > 0.0 => self,
            Self::WorldPoint(point) if point.is_finite() => self,
            Self::Selection(_) | Self::CameraTarget => self,
            Self::Distance(_) | Self::WorldPoint(_) => Self::CameraTarget,
        }
    }
}

/// Thin-lens depth-of-field settings for optical presentation.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub struct DepthOfField {
    /// Lens focal length, millimetres.
    pub focal_length_mm: f32,
    /// Aperture f-number.
    pub f_number: f32,
    /// Horizontal sensor extent, millimetres.
    pub sensor_width_mm: f32,
    /// Maximum circle-of-confusion radius, physical pixels.
    pub max_blur_pixels: f32,
    /// Aperture blade count, clamped to `[3, 12]`.
    pub blade_count: u8,
    /// Scene target from which the focal plane is resolved.
    pub focus: FocusTarget,
}

impl DepthOfField {
    /// A restrained full-frame macro-lens recipe for molecular close-ups.
    #[must_use]
    pub const fn macro_lens() -> Self {
        Self {
            focal_length_mm: 50.0,
            // Stopped well down on purpose. A molecule is a deep subject: at a
            // wide aperture its own front and back fall outside the focal
            // range and the whole specimen goes soft, which reads as a
            // low-resolution image rather than a photographic one. This keeps
            // the subject crisp and spends the blur on what is genuinely far
            // from the focal plane.
            f_number: 8.0,
            sensor_width_mm: 36.0,
            max_blur_pixels: 14.0,
            blade_count: 7,
            focus: FocusTarget::CameraTarget,
        }
    }

    pub(super) fn sanitize(self) -> Self {
        Self {
            focal_length_mm: finite_clamp(self.focal_length_mm, 1.0, 300.0, 50.0),
            f_number: finite_clamp(self.f_number, 0.7, 64.0, 4.0),
            sensor_width_mm: finite_clamp(self.sensor_width_mm, 1.0, 100.0, 36.0),
            max_blur_pixels: finite_clamp(self.max_blur_pixels, 0.0, 64.0, 0.0),
            blade_count: self.blade_count.clamp(3, 12),
            focus: self.focus.sanitize(),
        }
    }

    pub(super) fn blend(self, other: Self, weight: f32) -> Self {
        let weight = unit(weight);
        Self {
            focal_length_mm: lerp(self.focal_length_mm, other.focal_length_mm, weight),
            f_number: lerp(self.f_number, other.f_number, weight),
            sensor_width_mm: lerp(self.sensor_width_mm, other.sensor_width_mm, weight),
            max_blur_pixels: lerp(self.max_blur_pixels, other.max_blur_pixels, weight),
            blade_count: if weight >= 0.5 {
                other.blade_count
            } else {
                self.blade_count
            },
            focus: if weight >= 0.5 {
                other.focus
            } else {
                self.focus
            },
        }
        .sanitize()
    }

    pub(crate) fn packed(self, focus_distance: f32) -> [f32; 4] {
        let settings = self.sanitize();
        [
            focus_distance.max(1.0e-3),
            settings.focal_length_mm / (settings.f_number * settings.sensor_width_mm),
            settings.max_blur_pixels,
            f32::from(settings.blade_count),
        ]
    }
}

/// Camera-shutter motion blur driven by the renderer's true object motion.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub struct MotionBlur {
    /// Fraction of the frame interval exposed by the virtual shutter.
    pub shutter: f32,
    /// Maximum blur length in physical pixels.
    pub max_blur_pixels: f32,
}

impl MotionBlur {
    /// A restrained shutter that preserves molecular legibility.
    #[must_use]
    pub const fn restrained() -> Self {
        Self {
            shutter: 0.55,
            max_blur_pixels: 18.0,
        }
    }

    pub(super) fn sanitize(self) -> Self {
        Self {
            shutter: unit(self.shutter),
            max_blur_pixels: finite_clamp(self.max_blur_pixels, 0.0, 96.0, 18.0),
        }
    }

    pub(super) fn blend(self, other: Self, weight: f32) -> Self {
        let weight = unit(weight);
        Self {
            shutter: lerp(self.shutter, other.shutter, weight),
            max_blur_pixels: lerp(self.max_blur_pixels, other.max_blur_pixels, weight),
        }
        .sanitize()
    }

    pub(crate) fn packed(self) -> [f32; 4] {
        let value = self.sanitize();
        [value.shutter, value.max_blur_pixels, 0.0, 0.0]
    }
}
