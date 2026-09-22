//! Typed scalar-property descriptors and zero-copy runtime bindings.

pub(crate) mod registry;

#[cfg(test)]
mod tests;

use crate::{DataSource, Error, StructureId};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Stable reference shared by color and visual expressions.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct ScalarProperty {
    structure: StructureId,
    name: Box<str>,
}

impl ScalarProperty {
    /// Owning molecular structure.
    #[must_use]
    pub const fn structure(&self) -> StructureId {
        self.structure
    }

    /// Unique scene-level property name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
}

/// Serializable property metadata; values remain in a runtime binding.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct PropertySpec {
    /// Structure whose atom rows own the values.
    pub structure: StructureId,
    /// Portable bulk-data source.
    pub source: DataSource,
    /// Finite display domain in scientific units.
    pub domain: [f32; 2],
    /// Optional non-empty unit symbol.
    pub units: Option<Box<str>>,
}

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

    /// Attaches a scientific unit symbol.
    #[must_use]
    pub fn units(mut self, units: impl Into<Box<str>>) -> Self {
        self.spec.units = Some(units.into());
        self
    }

    pub(crate) fn validate(&self, rows: usize) -> Result<(), Error> {
        if self.name.trim().is_empty()
            || self.spec.source.content_hash.trim().is_empty()
            || self.values.len() != rows
            || self.values.iter().any(|value| value.is_infinite())
            || finite_domain(&self.values).is_none()
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
        let reference = ScalarProperty {
            structure: self.spec.structure,
            name: self.name,
        };
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

fn valid_domain(domain: [f32; 2]) -> bool {
    domain[0].is_finite() && domain[1].is_finite() && domain[0] < domain[1]
}

fn finite_domain(values: &[f32]) -> Option<[f32; 2]> {
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
