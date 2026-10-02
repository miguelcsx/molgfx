//! Zero-copy binding of a caller column to a property.

use super::{PropertySpec, ScalarProperty};
use crate::{DataSource, Error, StructureId};
use std::sync::Arc;

/// Immutable shared scalar column supplied outside [`crate::SceneSpec`].
#[derive(Clone, Debug)]
pub struct ScalarPropertyBinding {
    name: Box<str>,
    spec: PropertySpec,
    values: Arc<[f32]>,
}

impl ScalarPropertyBinding {
    /// Declares one atom-row scalar column without copying its values.
    #[must_use]
    pub fn new(
        structure: StructureId,
        name: impl Into<Box<str>>,
        source: DataSource,
        values: Arc<[f32]>,
    ) -> Self {
        let mut domain = [0.0, 1.0];
        if let Some(finite) = finite_domain(&values) {
            domain = finite;
        }
        Self {
            name: name.into(),
            spec: PropertySpec {
                structure,
                source,
                domain,
                units: None,
            },
            values,
        }
    }

    /// Overrides the finite display domain.
    #[must_use]
    pub fn domain(mut self, domain: [f32; 2]) -> Self {
        self.spec.domain = domain;
        self
    }

    /// Attaches a physical unit symbol.
    #[must_use]
    pub fn units(mut self, units: impl Into<Box<str>>) -> Self {
        self.spec.units = Some(units.into());
        self
    }

    pub(crate) fn validate(&self, rows: usize) -> Result<(), Error> {
        // A column with no finite value at all is allowed: it has nothing to
        // colour, so the renderer falls back to each atom's own colour. That is
        // how a structural scheme such as secondary structure or B-factor
        // degrades when the structure does not carry the property, instead of
        // rejecting an otherwise valid program.
        if self.name.trim().is_empty()
            || self.spec.source.content_hash.trim().is_empty()
            || self.values.len() != rows
            || self.values.iter().any(|value| value.is_infinite())
            || !valid_domain(self.spec.domain)
            || self
                .spec
                .units
                .as_deref()
                .is_some_and(|units| units.trim().is_empty())
        {
            return Err(Error::InvalidSpec(
                "property binding requires a name, source, matching atom rows, finite values or NaN missing values, a finite increasing domain, and non-empty units"
                    .to_owned(),
            ));
        }
        Ok(())
    }

    pub(crate) fn into_parts(self) -> (ScalarProperty, PropertySpec, Arc<[f32]>) {
        let reference = ScalarProperty::new(self.spec.structure, self.name);
        (reference, self.spec, self.values)
    }

    pub(crate) fn name(&self) -> &str {
        &self.name
    }

    pub(crate) const fn spec(&self) -> &PropertySpec {
        &self.spec
    }

    pub(crate) fn values(&self) -> &Arc<[f32]> {
        &self.values
    }
}

pub(super) fn valid_domain(domain: [f32; 2]) -> bool {
    domain[0].is_finite() && domain[1].is_finite() && domain[0] < domain[1]
}

pub(super) fn finite_domain(values: &[f32]) -> Option<[f32; 2]> {
    let mut finite = values.iter().copied().filter(|value| value.is_finite());
    let first = finite.next()?;
    let [low, high] = finite.fold([first, first], |[low, high], value| {
        [low.min(value), high.max(value)]
    });
    Some(if low < high {
        [low, high]
    } else {
        [low - 0.5, high + 0.5]
    })
}
