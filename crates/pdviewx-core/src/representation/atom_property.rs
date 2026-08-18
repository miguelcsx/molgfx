//! Immutable caller-supplied per-atom scalar properties and honest legends.

use crate::{AtomPropertyHandle, CoreError, ScalarFieldSemantics, StructureHandle};
use pdviewx_math::Rgba8;
use std::sync::Arc;

#[cfg(test)]
#[path = "atom_property_tests.rs"]
mod tests;

/// Scientific interpretation of one scalar atom column.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum AtomPropertyMeaning {
    /// Caller-defined scalar with semantics carried by its name and units.
    #[default]
    Generic,
    /// Confidence rank or calibrated confidence score, never an error radius.
    Confidence,
    /// Experimental/model occupancy.
    Occupancy,
    /// Caller-supplied local map resolution.
    LocalResolution,
    /// Caller-supplied flexibility such as RMSF or a B-factor rank.
    Flexibility,
    /// Charge or electrostatic scalar.
    Charge,
    /// Hydrophobicity scale named by provenance.
    Hydrophobicity,
    /// Solvent exposure or accessibility.
    Exposure,
}

/// One immutable scalar value per source atom row.
#[derive(Clone, PartialEq, Debug)]
pub struct AtomProperty {
    owner: StructureHandle,
    name: Arc<str>,
    values: Arc<[f32]>,
    meaning: AtomPropertyMeaning,
    semantics: ScalarFieldSemantics,
    finite_domain: [f32; 2],
}

impl AtomProperty {
    /// Creates a property without copying the caller's shared values.
    ///
    /// NaN denotes a missing value and is retained; infinities are rejected.
    /// Length is validated against the owning structure by
    /// [`crate::Scene::add_atom_property`].
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidProperty`] for an empty name, empty values,
    /// infinity, or a column with no finite value.
    pub fn new(
        owner: StructureHandle,
        name: impl Into<Arc<str>>,
        values: Arc<[f32]>,
        meaning: AtomPropertyMeaning,
        semantics: ScalarFieldSemantics,
    ) -> Result<Self, CoreError> {
        let name = name.into();
        if name.trim().is_empty() || values.is_empty() {
            return Err(invalid("property name and values must be non-empty"));
        }
        if values.iter().any(|value| value.is_infinite()) {
            return Err(invalid(
                "property values may be finite or missing NaN, never infinite",
            ));
        }
        let finite_domain = finite_domain(&values)
            .ok_or_else(|| invalid("property must contain at least one finite value"))?;
        Ok(Self {
            owner,
            name,
            values,
            meaning,
            semantics,
            finite_domain,
        })
    }

    /// Owning placed structure.
    #[must_use]
    pub const fn owner(&self) -> StructureHandle {
        self.owner
    }

    /// Caller-defined property label.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Shared values in source atom-row order.
    #[must_use]
    pub fn values(&self) -> &[f32] {
        &self.values
    }

    /// Shared backing allocation, useful to verify zero-copy handoff.
    #[must_use]
    pub fn shared_values(&self) -> &Arc<[f32]> {
        &self.values
    }

    /// Scientific interpretation.
    #[must_use]
    pub const fn meaning(&self) -> AtomPropertyMeaning {
        self.meaning
    }

    /// Rank or calibrated quantity metadata.
    #[must_use]
    pub const fn semantics(&self) -> &ScalarFieldSemantics {
        &self.semantics
    }

    /// Minimum and maximum over finite values only.
    #[must_use]
    pub const fn finite_domain(&self) -> [f32; 2] {
        self.finite_domain
    }

    /// Non-degenerate display domain used by reversible ramps.
    #[must_use]
    pub fn display_domain(&self) -> [f32; 2] {
        let [low, high] = self.finite_domain;
        if low < high {
            [low, high]
        } else {
            [low - 0.5, high + 0.5]
        }
    }

    /// Stable legend for a specific visual ramp.
    #[must_use]
    pub fn legend(&self, ramp: crate::ScalarRamp, missing: Rgba8) -> PropertyLegend {
        PropertyLegend {
            title: Arc::clone(&self.name),
            semantics: self.semantics.clone(),
            values: ramp.values(),
            colors: ramp.colors(),
            missing,
        }
    }
}

/// Reversible continuous legend emitted with a property encoding.
#[derive(Clone, PartialEq, Debug)]
pub struct PropertyLegend {
    /// Display title.
    pub title: Arc<str>,
    /// Quantity/units/provenance or explicit uncalibrated rank.
    pub semantics: ScalarFieldSemantics,
    /// Numeric stops in scientific units.
    pub values: [f32; 3],
    /// CVD-safe colours at those stops.
    pub colors: [Rgba8; 3],
    /// Colour used for missing values.
    pub missing: Rgba8,
}

/// Reversible scalar-to-opacity-and-edge-softness encoding.
///
/// This is an uncertainty presentation, not a spatial error model: it never
/// changes an atom radius or claims that softness is measured in Angstrom.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct PropertyAppearance {
    /// Property column sampled in source atom-row order.
    pub property: AtomPropertyHandle,
    domain: [f32; 2],
    opacity: [f32; 2],
    softness_pixels: [f32; 2],
    missing: PropertyAppearanceSample,
}

/// Resolved visual response for one property value.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct PropertyAppearanceSample {
    /// Opacity multiplier in `[0, 1]`.
    pub opacity: f32,
    /// Analytic silhouette transition width in physical pixels.
    pub softness_pixels: f32,
}

impl PropertyAppearance {
    /// Returns all endpoint and missing-value responses for serialization.
    #[must_use]
    pub const fn description_values(self) -> ([f32; 2], [f32; 2], [f32; 2], [f32; 2]) {
        (
            self.domain,
            self.opacity,
            self.softness_pixels,
            [self.missing.opacity, self.missing.softness_pixels],
        )
    }

    /// Creates a monotonic, invertible appearance mapping.
    ///
    /// # Errors
    ///
    /// The scientific domain must increase, endpoints must be finite and in
    /// range, and both visual channels must vary monotonically.
    pub fn new(
        property: AtomPropertyHandle,
        domain: [f32; 2],
        opacity: [f32; 2],
        softness_pixels: [f32; 2],
        missing: PropertyAppearanceSample,
    ) -> Result<Self, CoreError> {
        let finite = domain
            .into_iter()
            .chain(opacity)
            .chain(softness_pixels)
            .chain([missing.opacity, missing.softness_pixels])
            .all(f32::is_finite);
        if !finite || domain[0] >= domain[1] {
            return Err(invalid("appearance domain must be finite and increasing"));
        }
        if opacity
            .into_iter()
            .chain([missing.opacity])
            .any(|value| !(0.0..=1.0).contains(&value))
        {
            return Err(invalid("appearance opacity must be within zero and one"));
        }
        if softness_pixels
            .into_iter()
            .chain([missing.softness_pixels])
            .any(|value| !(0.0..=8.0).contains(&value))
        {
            return Err(invalid(
                "appearance softness must be within zero and eight pixels",
            ));
        }
        if (opacity[0] - opacity[1]).abs() < f32::EPSILON
            && (softness_pixels[0] - softness_pixels[1]).abs() < f32::EPSILON
        {
            return Err(invalid(
                "at least one appearance channel must encode the property",
            ));
        }
        Ok(Self {
            property,
            domain,
            opacity,
            softness_pixels,
            missing,
        })
    }

    /// Honest confidence-rank encoding: low values are diffuse and faint;
    /// high values are crisp and opaque.
    ///
    /// # Errors
    ///
    /// The supplied display domain must be finite and increasing.
    pub fn confidence(property: AtomPropertyHandle, domain: [f32; 2]) -> Result<Self, CoreError> {
        Self::new(
            property,
            domain,
            [0.28, 1.0],
            [2.4, 0.0],
            PropertyAppearanceSample {
                opacity: 0.22,
                softness_pixels: 2.8,
            },
        )
    }

    /// Flexibility-rank encoding: rigid values remain crisp while mobile
    /// values become diffuse and less opaque.
    ///
    /// # Errors
    ///
    /// The supplied display domain must be finite and increasing.
    pub fn flexibility(property: AtomPropertyHandle, domain: [f32; 2]) -> Result<Self, CoreError> {
        Self::new(
            property,
            domain,
            [1.0, 0.32],
            [0.0, 2.2],
            PropertyAppearanceSample {
                opacity: 0.24,
                softness_pixels: 2.6,
            },
        )
    }

    /// Scientific value interval represented by both visual channels.
    #[must_use]
    pub const fn domain(self) -> [f32; 2] {
        self.domain
    }

    /// Resolves a finite value, clamped to the declared domain; NaN uses the
    /// explicit missing response.
    #[must_use]
    pub fn sample(self, value: f32) -> PropertyAppearanceSample {
        if !value.is_finite() {
            return self.missing;
        }
        let parameter =
            ((value - self.domain[0]) / (self.domain[1] - self.domain[0])).clamp(0.0, 1.0);
        PropertyAppearanceSample {
            opacity: mix(self.opacity, parameter),
            softness_pixels: mix(self.softness_pixels, parameter),
        }
    }

    /// Recovers the represented scientific value from opacity when that
    /// channel varies.
    #[must_use]
    pub fn value_from_opacity(self, opacity: f32) -> Option<f32> {
        inverse(opacity, self.opacity, self.domain)
    }

    /// Recovers the represented scientific value from softness when that
    /// channel varies.
    #[must_use]
    pub fn value_from_softness(self, softness_pixels: f32) -> Option<f32> {
        inverse(softness_pixels, self.softness_pixels, self.domain)
    }

    /// True when any finite or missing sample can require alpha composition.
    #[must_use]
    pub fn is_translucent(self) -> bool {
        self.opacity[0] < 1.0
            || self.opacity[1] < 1.0
            || self.missing.opacity < 1.0
            || self.softness_pixels[0] > 0.0
            || self.softness_pixels[1] > 0.0
            || self.missing.softness_pixels > 0.0
    }
}

fn mix(range: [f32; 2], parameter: f32) -> f32 {
    range[0] + parameter * (range[1] - range[0])
}

fn inverse(value: f32, visual: [f32; 2], domain: [f32; 2]) -> Option<f32> {
    if !value.is_finite() || (visual[0] - visual[1]).abs() < f32::EPSILON {
        return None;
    }
    let parameter = ((value - visual[0]) / (visual[1] - visual[0])).clamp(0.0, 1.0);
    Some(mix(domain, parameter))
}

fn finite_domain(values: &[f32]) -> Option<[f32; 2]> {
    let mut domain: Option<[f32; 2]> = None;
    for &value in values.iter().filter(|value| value.is_finite()) {
        domain = Some(match domain {
            Some([low, high]) => [low.min(value), high.max(value)],
            None => [value, value],
        });
    }
    domain
}

const fn invalid(reason: &'static str) -> CoreError {
    CoreError::InvalidProperty { reason }
}
