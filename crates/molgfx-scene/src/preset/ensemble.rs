//! Weighted overlays of several structures and property-difference styling.

use crate::color::uniform;
use crate::visual::{ColorExpr, ScalarExpr, VisualStyle};
use crate::{Color, Error, RepresentationSpec, ScalarProperty, StructureId, rep, sel};
use std::collections::BTreeSet;

/// One structure of an ensemble overlay.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct EnsembleMember {
    /// The structure drawn for this member.
    pub structure: StructureId,
    /// Non-negative weight; only the ratios between members matter.
    pub weight: f32,
    /// The colour that identifies this member.
    pub color: Color,
}

/// Opacity policy for [`ensemble_representations`].
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct EnsembleStyle {
    /// Opacity of the highest-weight member.
    pub dominant_opacity: f32,
    /// Opacity of the other members when their weight equals the dominant one.
    pub alternate_opacity: f32,
    /// Floor that keeps a light member visible.
    pub minimum_opacity: f32,
}

impl Default for EnsembleStyle {
    fn default() -> Self {
        Self {
            dominant_opacity: 1.0,
            alternate_opacity: 0.55,
            minimum_opacity: 0.08,
        }
    }
}

impl EnsembleStyle {
    fn validate(self) -> Result<(), Error> {
        let unit = |value: f32| value.is_finite() && (0.0..=1.0).contains(&value);
        let ordered = self.minimum_opacity <= self.alternate_opacity;
        if unit(self.dominant_opacity)
            && unit(self.alternate_opacity)
            && unit(self.minimum_opacity)
            && ordered
        {
            Ok(())
        } else {
            Err(Error::InvalidSpec(
                "ensemble opacities must lie in zero to one with minimum <= alternate".to_owned(),
            ))
        }
    }
}

/// Draws each member as a cartoon in its own colour, the heaviest most opaque.
///
/// Weights are normalized, the member with the largest weight (the first on a
/// tie) draws at `dominant_opacity`, and every other member's opacity is its
/// weight relative to the dominant one times `alternate_opacity`, clamped to
/// `[minimum_opacity, alternate_opacity]`. The result is ordinary
/// representation specifications, one per member in input order.
///
/// # Errors
///
/// Returns an error for no members, a repeated structure, a negative or
/// non-finite weight, weights that sum to zero, or an invalid opacity.
pub fn ensemble_representations(
    members: &[EnsembleMember],
    style: EnsembleStyle,
) -> Result<Vec<RepresentationSpec>, Error> {
    style.validate()?;
    if members.is_empty() {
        return Err(Error::InvalidSpec(
            "an ensemble needs at least one member".to_owned(),
        ));
    }
    let mut seen = BTreeSet::new();
    for member in members {
        if !seen.insert(member.structure) {
            return Err(Error::InvalidSpec(format!(
                "structure {} appears twice in the ensemble",
                member.structure.0
            )));
        }
        if !member.weight.is_finite() || member.weight < 0.0 {
            return Err(Error::InvalidSpec(
                "ensemble weights must be finite and non-negative".to_owned(),
            ));
        }
    }
    let total = members.iter().map(|member| member.weight).sum::<f32>();
    if total <= 0.0 {
        return Err(Error::InvalidSpec(
            "ensemble weights must not all be zero".to_owned(),
        ));
    }
    let dominant = members.iter().enumerate().fold(0, |best, (index, member)| {
        if member.weight > members[best].weight {
            index
        } else {
            best
        }
    });
    let heaviest = members[dominant].weight;
    Ok(members
        .iter()
        .enumerate()
        .map(|(index, member)| {
            let opacity = if index == dominant {
                style.dominant_opacity
            } else {
                (member.weight / heaviest * style.alternate_opacity)
                    .clamp(style.minimum_opacity, style.alternate_opacity)
            };
            rep::cartoon(sel::polymer())
                .opacity(opacity)
                .color(uniform(member.color))
                .structure(member.structure)
                .into()
        })
        .collect())
}

/// Thresholds and colours for [`difference_visual`].
#[derive(Clone, PartialEq, Debug)]
pub struct DifferenceStyle {
    /// Values at or below this stay context.
    pub context_threshold: f32,
    /// Value at which emphasis reaches full opacity.
    pub emphasis_threshold: f32,
    /// Opacity of context.
    pub context_opacity: f32,
    /// Named palette the property is coloured by.
    pub palette: String,
    /// Scalar range the palette spans.
    pub domain: [f32; 2],
    /// Colour for atoms without a value.
    pub missing: Color,
}

impl Default for DifferenceStyle {
    fn default() -> Self {
        Self {
            context_threshold: 0.0,
            emphasis_threshold: 1.0,
            context_opacity: 0.16,
            palette: "viridis".to_owned(),
            domain: [0.0, 1.0],
            missing: Color::rgb(128, 128, 128),
        }
    }
}

/// A visual style that colours by `property` and fades small values to context.
///
/// Opacity rises linearly from `context_opacity` at `context_threshold` to one
/// at `emphasis_threshold`. The property is the caller's difference, score or
/// uncertainty column, bound to the scene beforehand.
///
/// # Errors
///
/// Returns an error for non-finite or unordered thresholds, a degenerate
/// domain, or a context opacity outside zero to one.
pub fn difference_visual(
    property: ScalarProperty,
    style: &DifferenceStyle,
) -> Result<VisualStyle, Error> {
    let finite = style.context_threshold.is_finite()
        && style.emphasis_threshold.is_finite()
        && style.domain.iter().all(|value| value.is_finite());
    if !finite || style.context_threshold >= style.emphasis_threshold {
        return Err(Error::InvalidSpec(
            "difference thresholds must be finite and satisfy context < emphasis".to_owned(),
        ));
    }
    if style.domain[0] >= style.domain[1] {
        return Err(Error::InvalidSpec(
            "the difference palette domain must be increasing".to_owned(),
        ));
    }
    if !style.context_opacity.is_finite() || !(0.0..=1.0).contains(&style.context_opacity) {
        return Err(Error::InvalidSpec(
            "the difference context opacity must lie in zero to one".to_owned(),
        ));
    }
    let span = style.emphasis_threshold - style.context_threshold;
    let weight = ScalarExpr::Multiply(
        ScalarExpr::Add(
            ScalarExpr::Property(property.clone()).into(),
            ScalarExpr::Constant(-style.context_threshold).into(),
        )
        .into(),
        ScalarExpr::Constant(1.0 / span).into(),
    )
    .clamp(0.0, 1.0);
    let opacity = ScalarExpr::Add(
        ScalarExpr::Constant(style.context_opacity).into(),
        ScalarExpr::Multiply(
            ScalarExpr::Constant(1.0 - style.context_opacity).into(),
            weight.into(),
        )
        .into(),
    );
    let color = ColorExpr::Ramp {
        value: ScalarExpr::Property(property).into(),
        palette: style.palette.as_str().into(),
        domain: style.domain,
        missing: style.missing,
    };
    Ok(VisualStyle::new(
        color,
        opacity,
        crate::visual::BoolExpr::Constant(true),
    ))
}
