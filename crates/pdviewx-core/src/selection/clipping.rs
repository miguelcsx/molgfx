//! Fixed-capacity per-representation clipping and slabs.

use crate::CoreError;
use pdviewx_math::Vec3;

#[cfg(test)]
#[path = "clipping_tests.rs"]
mod tests;

/// Portable maximum shared by every representation shader.
pub const MAX_CLIP_PLANES: usize = 4;

/// Treatment of molecular interiors exposed by clipping.
#[repr(u32)]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ClipCap {
    /// Leave the cut open and show only retained natural boundaries.
    #[default]
    Open = 0,
    /// Draw a flat, depth-correct cross-section where a plane cuts matter.
    Solid = 1,
}

/// A world-space half-space; non-negative signed distance remains visible.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ClipPlane {
    /// Unit normal pointing toward the retained half-space.
    pub normal: Vec3,
    /// Plane equation offset: `dot(normal, point) + offset`.
    pub offset: f32,
}

impl ClipPlane {
    /// Builds a normalized plane through a world-space point.
    ///
    /// # Errors
    ///
    /// The point and normal must be finite and the normal non-zero.
    pub fn from_point_normal(point: Vec3, normal: Vec3) -> Result<Self, CoreError> {
        if !point.is_finite() || !normal.is_finite() || normal.length_squared() <= 1e-12 {
            return Err(invalid("clip point and non-zero normal must be finite"));
        }
        let normal = normal.normalize();
        Ok(Self {
            normal,
            offset: -normal.dot(point),
        })
    }

    /// Reverses which half-space remains visible.
    #[must_use]
    pub fn reversed(self) -> Self {
        Self {
            normal: -self.normal,
            offset: -self.offset,
        }
    }

    /// Signed distance; non-negative points remain visible.
    #[must_use]
    pub fn signed_distance(self, point: Vec3) -> f32 {
        self.normal.dot(point) + self.offset
    }
}

/// Up to four planes applied only to one representation.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ClipSet {
    planes: [ClipPlane; MAX_CLIP_PLANES],
    len: u8,
    cap: ClipCap,
}

impl ClipSet {
    /// Creates a set from zero to four planes.
    ///
    /// # Errors
    ///
    /// More than four planes exceed the portable uniform contract.
    pub fn new(planes: &[ClipPlane]) -> Result<Self, CoreError> {
        if planes.len() > MAX_CLIP_PLANES {
            return Err(invalid("at most four clip planes are portable"));
        }
        let mut set = Self::default();
        set.planes[..planes.len()].copy_from_slice(planes);
        set.len = u8::try_from(planes.len()).map_or(0, |len| len);
        Ok(set)
    }

    /// Creates opposing planes retaining a slab of positive thickness.
    ///
    /// # Errors
    ///
    /// Thickness must be positive and finite; plane validation also applies.
    pub fn slab(center: Vec3, normal: Vec3, thickness: f32) -> Result<Self, CoreError> {
        if !thickness.is_finite() || thickness <= 0.0 {
            return Err(invalid("slab thickness must be positive and finite"));
        }
        if !normal.is_finite() || normal.length_squared() <= 1e-12 {
            return Err(invalid("slab normal must be finite and non-zero"));
        }
        let direction = normal.normalize();
        let half = thickness * 0.5;
        Self::new(&[
            ClipPlane::from_point_normal(center - direction * half, direction)?,
            ClipPlane::from_point_normal(center + direction * half, -direction)?,
        ])
    }

    /// Active planes in declaration order.
    #[must_use]
    pub fn planes(&self) -> &[ClipPlane] {
        &self.planes[..usize::from(self.len)]
    }

    /// Whether a world-space point survives every plane.
    #[must_use]
    pub fn contains(&self, point: Vec3) -> bool {
        self.planes()
            .iter()
            .all(|plane| plane.signed_distance(point) >= 0.0)
    }

    /// Chooses whether clipped molecular interiors receive solid caps.
    #[must_use]
    pub fn with_cap(mut self, cap: ClipCap) -> Self {
        self.cap = cap;
        self
    }

    /// Current cap behavior.
    #[must_use]
    pub const fn cap(&self) -> ClipCap {
        self.cap
    }
}

impl Default for ClipSet {
    fn default() -> Self {
        let plane = ClipPlane {
            normal: Vec3::X,
            offset: 0.0,
        };
        Self {
            planes: [plane; MAX_CLIP_PLANES],
            len: 0,
            cap: ClipCap::Open,
        }
    }
}

const fn invalid(reason: &'static str) -> CoreError {
    CoreError::InvalidClip { reason }
}
