//! Caller-authored guide geometry: plain lines and arrows.
//!
//! An interaction edge states a domain claim — this is a hydrogen bond,
//! that is a salt bridge — and takes its whole appearance from that claim. A
//! guide makes no claim. It is the arrow that points at a pocket in a figure,
//! the axis of a helix, the segment marking a distance a reader should notice.
//! Keeping it a separate type is what stops a decorative arrow from being read
//! later as detected chemistry.
//!
//! Guides draw through the same analytic glyph path as interactions, so they
//! inherit its pixel-stable marks, transparency, depth and picking without a
//! second pass or a second shader.

use crate::error::CoreError;
use crate::handle::StructureHandle;
use crate::representation::relation::RelationPattern;
use molgfx_math::{Rgba8, Vec3};

#[cfg(test)]
#[path = "guide_tests.rs"]
mod tests;

/// How a guide's ends are marked.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum GuideCap {
    /// A plain segment.
    #[default]
    None,
    /// An arrowhead at the end point.
    Arrow,
    /// Arrowheads at both ends, for a span or a measurement.
    DoubleArrow,
}

/// Whether a polyline stops at its final point or closes back to its first.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum PolylineKind {
    /// Consecutive points form an open path.
    #[default]
    Open,
    /// The final point is connected back to the first.
    Closed,
}

/// Presentation of one guide. Every field is caller state; nothing is derived
/// from chemistry.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct GuideStyle {
    /// Line colour.
    pub color: Rgba8,
    /// Screen-space mark vocabulary shared with generic relation glyphs.
    pub pattern: RelationPattern,
    /// Pixel-stable line width.
    pub width_pixels: f32,
    /// Final alpha before weighted transparency.
    pub opacity: f32,
    /// Repetition period in pixels; ignored by solid lines.
    pub period_pixels: f32,
    /// Fraction of each period occupied by a mark.
    pub duty_cycle: f32,
    /// End decoration.
    pub cap: GuideCap,
    /// Arrowhead size in pixels.
    pub arrow_pixels: f32,
}

impl Default for GuideStyle {
    fn default() -> Self {
        Self {
            color: Rgba8::opaque(226, 232, 240),
            pattern: RelationPattern::Solid,
            width_pixels: 1.6,
            opacity: 1.0,
            period_pixels: 8.0,
            duty_cycle: 0.5,
            cap: GuideCap::None,
            arrow_pixels: 9.0,
        }
    }
}

impl GuideStyle {
    /// Finite, bounded values consumed by the glyph pass.
    #[must_use]
    pub fn sanitized(self) -> Self {
        let default = Self::default();
        Self {
            color: self.color,
            pattern: self.pattern,
            width_pixels: bounded(self.width_pixels, 0.1, 64.0, default.width_pixels),
            opacity: bounded(self.opacity, 0.0, 1.0, default.opacity),
            period_pixels: bounded(self.period_pixels, 1.0, 256.0, default.period_pixels),
            duty_cycle: bounded(self.duty_cycle, 0.05, 1.0, default.duty_cycle),
            cap: self.cap,
            arrow_pixels: bounded(self.arrow_pixels, 0.0, 64.0, default.arrow_pixels),
        }
    }
}

/// One caller-authored guide owned by a structure, so it follows that
/// structure's placement.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Guide {
    owner: StructureHandle,
    start: Vec3,
    end: Vec3,
    style: GuideStyle,
    visible: bool,
}

impl Guide {
    /// Creates a guide between two finite, distinct model-space points.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidAnnotation`] when a point is not finite or
    /// the two coincide, which would leave the direction undefined.
    pub fn new(
        owner: StructureHandle,
        start: Vec3,
        end: Vec3,
        style: GuideStyle,
    ) -> Result<Self, CoreError> {
        if !start.is_finite() || !end.is_finite() {
            return Err(CoreError::InvalidAnnotation {
                reason: "guide endpoints must be finite",
            });
        }
        if start.distance_squared(end) <= f32::EPSILON {
            return Err(CoreError::InvalidAnnotation {
                reason: "guide endpoints must be distinct",
            });
        }
        Ok(Self {
            owner,
            start,
            end,
            style: style.sanitized(),
            visible: true,
        })
    }

    /// The structure whose placement carries this guide.
    #[must_use]
    pub const fn owner(&self) -> StructureHandle {
        self.owner
    }

    /// Start point in the owner's model space.
    #[must_use]
    pub const fn start(&self) -> Vec3 {
        self.start
    }

    /// End point in the owner's model space.
    #[must_use]
    pub const fn end(&self) -> Vec3 {
        self.end
    }

    /// Resolved presentation.
    #[must_use]
    pub const fn style(&self) -> GuideStyle {
        self.style
    }

    /// Whether the guide draws.
    #[must_use]
    pub const fn visible(&self) -> bool {
        self.visible
    }

    /// Shows or hides the guide without discarding it.
    pub const fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    /// Replaces the presentation, sanitizing it.
    pub fn set_style(&mut self, style: GuideStyle) {
        self.style = style.sanitized();
    }
}

fn bounded(value: f32, minimum: f32, maximum: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value.clamp(minimum, maximum)
    } else {
        fallback
    }
}
