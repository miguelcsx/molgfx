//! The built-in color themes.

use super::{AtomCategory, AtomMetric, Color, ColorSpec};

/// Colors by element.
#[must_use]
pub const fn element() -> ColorSpec {
    ColorSpec::Element
}

/// Colors by chain.
#[must_use]
pub const fn chain() -> ColorSpec {
    ColorSpec::category(AtomCategory::Chain)
}

/// Colors by entity, the distinct molecular species.
#[must_use]
pub const fn entity() -> ColorSpec {
    ColorSpec::category(AtomCategory::Entity)
}

/// Colors by molecule type: water, ion, protein, RNA, DNA, saccharide.
#[must_use]
pub const fn molecule_type() -> ColorSpec {
    ColorSpec::category(AtomCategory::MoleculeType)
}

/// Colors amino acids and nucleotides by chemistry.
#[must_use]
pub const fn residue_name() -> ColorSpec {
    ColorSpec::category(AtomCategory::ResidueName)
}

/// Stable categorical colors by residue.
#[must_use]
pub const fn residue() -> ColorSpec {
    ColorSpec::category(AtomCategory::Residue)
}

/// Colors by secondary-structure state.
#[must_use]
pub const fn secondary_structure() -> ColorSpec {
    ColorSpec::category(AtomCategory::SecondaryStructure)
}

/// Colors only carbon atoms by chain and leaves every other element its own
/// colour, the convention of most molecular viewers.
#[must_use]
pub const fn carbon_by_chain() -> ColorSpec {
    ColorSpec::Category {
        by: AtomCategory::Chain,
        palette: None,
        carbon_only: true,
    }
}

/// Uses one color everywhere.
#[must_use]
pub const fn uniform(color: Color) -> ColorSpec {
    ColorSpec::Uniform { color }
}

/// Colors by a value the structure defines for itself, on that metric's own
/// ramp and domain.
#[must_use]
pub const fn metric(metric: AtomMetric) -> ColorSpec {
    ColorSpec::Metric {
        metric,
        ramp: None,
        domain: None,
    }
}

/// Maps a scalar molecular property through an explicit physical domain.
#[must_use]
pub fn property(
    property: crate::ScalarProperty,
    ramp: impl Into<Box<str>>,
    domain: [f32; 2],
    units: Option<Box<str>>,
    missing: Color,
) -> ColorSpec {
    ColorSpec::Property {
        property,
        ramp: ramp.into(),
        domain,
        units,
        missing,
    }
}
