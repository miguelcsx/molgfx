//! Caller-provided validation findings lowered to annotation markers.

use crate::{AnnotationAnchor, CoreError, MarkerStyle, StructureHandle};

/// Caller-provided validation finding category.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[non_exhaustive]
pub enum ValidationKind {
    /// Steric overlap or other atom-pair clash.
    #[default]
    Clash,
    /// Geometry outlier such as a bond or angle deviation.
    Geometry,
    /// Density-fit or map agreement outlier.
    Density,
    /// A caller-defined validation finding.
    Other,
}

/// One validation marker ready to lower into the annotation/marker path.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ValidationMarker {
    /// Structure that owns the marker.
    pub owner: StructureHandle,
    /// Position and optional source entity.
    pub anchor: AnnotationAnchor,
    /// Finding category supplied by the validator.
    pub kind: ValidationKind,
    /// Normalized severity in `[0, 1]`.
    pub severity: f32,
    /// Marker presentation.
    pub style: MarkerStyle,
}

impl ValidationMarker {
    /// Creates one validated caller finding; no clash detection is performed.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidPrimitive`] for an invalid
    /// severity or marker style.
    pub fn new(
        owner: StructureHandle,
        anchor: AnnotationAnchor,
        kind: ValidationKind,
        severity: f32,
        style: MarkerStyle,
    ) -> Result<Self, CoreError> {
        if !severity.is_finite() || !(0.0..=1.0).contains(&severity) {
            return Err(invalid("validation severity must be within [0, 1]"));
        }
        if !style.radius_pixels.is_finite() || !(1.0..=64.0).contains(&style.radius_pixels) {
            return Err(invalid(
                "validation marker radius must be within 1 to 64 pixels",
            ));
        }
        Ok(Self {
            owner,
            anchor,
            kind,
            severity,
            style,
        })
    }
}

const fn invalid(reason: &'static str) -> CoreError {
    CoreError::InvalidPrimitive { reason }
}
