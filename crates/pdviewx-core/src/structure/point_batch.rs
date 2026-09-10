//! Compact generic points with one shared glyph and layout per batch.

#[cfg(test)]
#[path = "point_batch_tests.rs"]
mod tests;

use crate::{CoreError, SourceRows};
use pdviewx_math::{Aabb, Rgba8, Vec3};
use std::sync::Arc;

const PARALLEL_BOUNDS_THRESHOLD: usize = pdviewx_math::parallel::BLOCK * 4;

/// Homogeneous analytic glyph used by every point in a batch.
#[repr(u32)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum PointGlyph {
    /// Camera-facing analytic disc.
    #[default]
    Disc = 0,
    /// Analytic sphere impostor.
    Sphere = 1,
}

/// Batch-wide fallback appearance. Per-row variation belongs in attributes.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct PointStyle {
    /// World-space glyph radius.
    pub radius: f32,
    /// Packed fallback color.
    pub color: Rgba8,
}

impl Default for PointStyle {
    fn default() -> Self {
        Self {
            radius: 0.1,
            color: Rgba8::WHITE,
        }
    }
}

/// Immutable point payload prepared outside a scene.
#[derive(Clone, PartialEq, Debug)]
pub struct PointBatch {
    positions: Arc<[[f32; 3]]>,
    source_rows: SourceRows,
    glyph: PointGlyph,
    style: PointStyle,
    bounds: Aabb,
    visible: bool,
}

impl PointBatch {
    /// Retains tightly packed 12-byte positions without expanding templates.
    ///
    /// Bounds use deterministic 8192-row blocks above the parallel threshold.
    ///
    /// # Errors
    ///
    /// Positions must be finite, non-empty and match `source_rows` exactly.
    pub fn new(
        positions: Arc<[[f32; 3]]>,
        source_rows: SourceRows,
        glyph: PointGlyph,
        style: PointStyle,
    ) -> Result<Self, CoreError> {
        if positions.is_empty() || positions.len() != source_rows.len() as usize {
            return Err(invalid(
                "point positions must be non-empty and match their source rows",
            ));
        }
        if !style.radius.is_finite() || style.radius <= 0.0 {
            return Err(invalid("point radius must be finite and positive"));
        }
        if !pdviewx_math::parallel::all_blocks(&positions, PARALLEL_BOUNDS_THRESHOLD, |block| {
            block
                .iter()
                .all(|position| position.iter().all(|value| value.is_finite()))
        }) {
            return Err(invalid("point positions must be finite"));
        }
        let bounds = point_bounds(&positions, style.radius);
        Ok(Self {
            positions,
            source_rows,
            glyph,
            style,
            bounds,
            visible: true,
        })
    }

    /// Shared 12-byte rows in logical order.
    #[must_use]
    pub const fn positions(&self) -> &Arc<[[f32; 3]]> {
        &self.positions
    }

    /// External source identity.
    #[must_use]
    pub const fn source_rows(&self) -> &SourceRows {
        &self.source_rows
    }

    /// Shared glyph kind.
    #[must_use]
    pub const fn glyph(&self) -> PointGlyph {
        self.glyph
    }

    /// Shared fallback style.
    #[must_use]
    pub const fn style(&self) -> PointStyle {
        self.style
    }

    /// Conservative world-space bounds.
    #[must_use]
    pub const fn bounds(&self) -> Aabb {
        self.bounds
    }

    /// Current scene visibility.
    #[must_use]
    pub const fn visible(&self) -> bool {
        self.visible
    }

    pub(crate) fn set_visible(&mut self, visible: bool) -> bool {
        if self.visible == visible {
            return false;
        }
        self.visible = visible;
        true
    }
}

fn point_bounds(positions: &[[f32; 3]], radius: f32) -> Aabb {
    pdviewx_math::parallel::reduce_blocks(
        positions,
        PARALLEL_BOUNDS_THRESHOLD,
        Aabb::EMPTY,
        |block| {
            let mut bound = Aabb::EMPTY;
            for position in block {
                bound.extend_sphere(Vec3::from(position), radius);
            }
            bound
        },
        |left, right| left.union(&right),
    )
}

const fn invalid(reason: &'static str) -> CoreError {
    CoreError::InvalidBatch { reason }
}
