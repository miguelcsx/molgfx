//! Scalar columns the structure defines for itself, bound on demand.
//!
//! A colour theme such as "by chain" or "by hydropathy" needs one number per
//! atom. The number is a pure function of the structure, so the scene derives
//! it the first time a colour asks for it, binds it through the same scalar
//! property path as any caller-supplied column, and caches it by name. The
//! renderer, the geometry and the serialized scene therefore see an ordinary
//! property; only its descriptor says it is derived, so a replica can rebuild
//! it from the structure instead of receiving it.

mod category;
mod metric;

pub use category::AtomCategory;
pub use metric::AtomMetric;

use crate::{DataSource, Error, ScalarPropertyBinding, StructureId};
use molgfx_core::MolecularSource;
use std::sync::Arc;

/// Marks a property descriptor as derived from its structure.
pub(crate) const DERIVED_FORMAT: &str = "derived";

/// One derived column.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum DerivedColumn {
    /// A quantitative value: occupancy, temperature factor, hydropathy.
    Metric(AtomMetric),
    /// A whole-number category: chain, entity, molecule type.
    Category {
        /// What atoms are categorised by.
        by: AtomCategory,
        /// Whether only carbon atoms carry a category.
        carbon_only: bool,
    },
}

/// The atomic number of carbon.
const CARBON: u8 = 6;

impl DerivedColumn {
    /// The stable name, unique among derived columns.
    #[must_use]
    pub fn name(self) -> String {
        match self {
            Self::Metric(metric) => format!("metric.{}", metric.name()),
            Self::Category {
                by,
                carbon_only: false,
            } => format!("category.{}", by.name()),
            Self::Category {
                by,
                carbon_only: true,
            } => format!("category.{}.carbon", by.name()),
        }
    }

    /// The column a name refers to.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        if let Some(metric) = name.strip_prefix("metric.") {
            return AtomMetric::ALL
                .into_iter()
                .find(|known| known.name() == metric)
                .map(Self::Metric);
        }
        let category = name.strip_prefix("category.")?;
        let (base, carbon_only) = match category.strip_suffix(".carbon") {
            Some(base) => (base, true),
            None => (category, false),
        };
        AtomCategory::from_name(base).map(|by| Self::Category { by, carbon_only })
    }

    /// The property name this column is bound under in `structure`.
    ///
    /// The `@` keeps derived names from ever colliding with a caller's own.
    #[must_use]
    pub fn property_name(self, structure: StructureId) -> String {
        format!("{}.@{}", structure.get(), self.name())
    }

    /// Computes the column for `source`, one value per atom.
    ///
    /// # Errors
    ///
    /// Returns an invalid-specification error when the source cannot supply
    /// what the column reads.
    pub fn column(self, source: &MolecularSource) -> Result<Vec<f32>, Error> {
        match self {
            Self::Metric(metric) => metric.column(source),
            Self::Category { by, carbon_only } => {
                let mut values = by.column(source)?;
                if carbon_only {
                    for (value, atom) in values.iter_mut().zip(source.topology().atoms.iter()) {
                        if atom.element != CARBON {
                            *value = f32::NAN;
                        }
                    }
                }
                Ok(values)
            }
        }
    }

    /// Derives this column for `structure` as a scalar property binding.
    ///
    /// # Errors
    ///
    /// Returns an invalid-specification error when the column cannot be
    /// derived.
    pub fn binding(
        self,
        structure: StructureId,
        source: &MolecularSource,
    ) -> Result<ScalarPropertyBinding, Error> {
        let values: Arc<[f32]> = self.column(source)?.into();
        let name = self.name();
        let hash = crate::structure_hash(source);
        let binding = ScalarPropertyBinding::new(
            structure,
            self.property_name(structure),
            DataSource::new(format!("derived:{name}:{hash}"))
                .format(DERIVED_FORMAT)
                .uri(name),
            values,
        );
        Ok(match self {
            Self::Metric(metric) => {
                let binding = binding.domain(metric.domain());
                match metric.units() {
                    Some(units) => binding.units(units),
                    None => binding,
                }
            }
            Self::Category { by, .. } => {
                // A categorical column is indexed by whole-number category, so
                // its domain is the palette's index range rather than its
                // values. That keeps the legend correct and lets a structure
                // with no category at all — every value missing, as when a
                // file carries no secondary structure — still validate.
                let categories = by.default_palette().len().max(2);
                binding.domain([0.0, (categories - 1) as f32])
            }
        })
    }
}
