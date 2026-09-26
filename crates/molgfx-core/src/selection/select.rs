//! Typed selection IR for world-space and topology predicates.

use crate::{CoreError, SecondaryStructure};
use molgfx_math::Vec3;

#[cfg(test)]
#[path = "select_tests.rs"]
mod tests;

/// One validated, storage-independent molecular selection expression.
#[derive(Clone, PartialEq, Debug)]
pub struct Select(pub(crate) SelectExpr);

#[derive(Clone, PartialEq, Debug)]
pub(crate) enum SelectExpr {
    Class(EntityClass),
    Predicate(AtomPredicate),
    InSphere {
        center: Vec3,
        radius: f32,
    },
    InBox {
        min: Vec3,
        max: Vec3,
    },
    Within {
        distance: f32,
        reference: Box<SelectExpr>,
        residues: bool,
    },
    And(Box<SelectExpr>, Box<SelectExpr>),
    Or(Box<SelectExpr>, Box<SelectExpr>),
    Not(Box<SelectExpr>),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum EntityClass {
    All,
    None,
    Polymer,
    Protein,
    Nucleic,
    NonPolymer,
    Water,
    Branched,
}

#[derive(Clone, PartialEq, Debug)]
pub(crate) enum AtomPredicate {
    Chain(Box<str>),
    ResidueName(Box<str>),
    AtomName(Box<str>),
    ResidueNumber(i32),
    Element(u8),
    Secondary(SecondaryStructure),
    Scalar {
        property: ScalarProperty,
        comparison: PropertyComparison,
        threshold: f32,
    },
    Hydrogen,
    Heavy,
    Backbone,
    Terminus,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum ScalarProperty {
    BFactor,
    Occupancy,
}

/// Comparison used by numeric atom-property predicates.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PropertyComparison {
    /// Strictly less than.
    Less,
    /// Less than or equal to.
    LessOrEqual,
    /// Equal within the source `f32` representation.
    Equal,
    /// Greater than or equal to.
    GreaterOrEqual,
    /// Strictly greater than.
    Greater,
}

impl PropertyComparison {
    pub(crate) const fn matches(self, value: f32, threshold: f32) -> bool {
        match self {
            Self::Less => value < threshold,
            Self::LessOrEqual => value <= threshold,
            Self::Equal => value.to_bits() == threshold.to_bits(),
            Self::GreaterOrEqual => value >= threshold,
            Self::Greater => value > threshold,
        }
    }
}

impl Select {
    /// Every atom in every placed structure.
    #[must_use]
    pub const fn all() -> Self {
        Self(SelectExpr::Class(EntityClass::All))
    }

    /// The empty selection.
    #[must_use]
    pub const fn none() -> Self {
        Self(SelectExpr::Class(EntityClass::None))
    }

    /// All declared polymer entities.
    #[must_use]
    pub const fn polymer() -> Self {
        Self(SelectExpr::Class(EntityClass::Polymer))
    }

    /// Declared polypeptide chains.
    #[must_use]
    pub const fn protein() -> Self {
        Self(SelectExpr::Class(EntityClass::Protein))
    }

    /// Declared DNA, RNA and nucleic-hybrid chains.
    #[must_use]
    pub const fn nucleic() -> Self {
        Self(SelectExpr::Class(EntityClass::Nucleic))
    }

    /// Declared non-polymer entities, including standalone ions.
    #[must_use]
    pub const fn ligands() -> Self {
        Self(SelectExpr::Class(EntityClass::NonPolymer))
    }

    /// Declared water entities.
    #[must_use]
    pub const fn water() -> Self {
        Self(SelectExpr::Class(EntityClass::Water))
    }

    /// Declared branched entities such as oligosaccharides.
    #[must_use]
    pub const fn branched() -> Self {
        Self(SelectExpr::Class(EntityClass::Branched))
    }

    /// Atoms belonging to a label or author chain identifier.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidSelection`] when `label` is empty.
    pub fn chain(label: impl AsRef<str>) -> Result<Self, CoreError> {
        text_predicate(label, AtomPredicate::Chain, "chain label must not be empty")
    }

    /// Atoms in a residue component, such as `ALA` or `ATP`.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidSelection`] when `name` is empty.
    pub fn residue_name(name: impl AsRef<str>) -> Result<Self, CoreError> {
        text_predicate(
            name,
            AtomPredicate::ResidueName,
            "residue name must not be empty",
        )
    }

    /// Atoms with a deposited atom name, such as `CA` or `O1P`.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidSelection`] when `name` is empty.
    pub fn atom_name(name: impl AsRef<str>) -> Result<Self, CoreError> {
        text_predicate(name, AtomPredicate::AtomName, "atom name must not be empty")
    }

    /// Atoms in a residue carrying either namespace's sequence number.
    #[must_use]
    pub const fn residue(number: i32) -> Self {
        Self(SelectExpr::Predicate(AtomPredicate::ResidueNumber(number)))
    }

    /// Atoms of one element, supplied as a chemical symbol.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidSelection`] for an unknown symbol.
    pub fn element(symbol: &str) -> Result<Self, CoreError> {
        let Some(element) = molframe::Element::from_symbol(symbol) else {
            return Err(invalid("element symbol is unknown"));
        };
        Ok(Self(SelectExpr::Predicate(AtomPredicate::Element(
            element.atomic_number(),
        ))))
    }

    /// Atoms assigned to one caller- or `molframe`-supplied secondary class.
    #[must_use]
    pub const fn secondary(value: SecondaryStructure) -> Self {
        Self(SelectExpr::Predicate(AtomPredicate::Secondary(value)))
    }

    /// Helical residues.
    #[must_use]
    pub const fn helix() -> Self {
        Self::secondary(SecondaryStructure::Helix)
    }

    /// Beta-strand residues.
    #[must_use]
    pub const fn sheet() -> Self {
        Self::secondary(SecondaryStructure::Strand)
    }

    /// Coil residues.
    #[must_use]
    pub const fn coil() -> Self {
        Self::secondary(SecondaryStructure::Coil)
    }

    /// Hydrogen atoms.
    #[must_use]
    pub const fn hydrogen() -> Self {
        Self(SelectExpr::Predicate(AtomPredicate::Hydrogen))
    }

    /// Non-hydrogen atoms with a declared element.
    #[must_use]
    pub const fn heavy() -> Self {
        Self(SelectExpr::Predicate(AtomPredicate::Heavy))
    }

    /// Conventional polymer backbone atom names.
    #[must_use]
    pub const fn backbone() -> Self {
        Self(SelectExpr::Predicate(AtomPredicate::Backbone))
    }

    /// Atoms in the first or last residue of a chain.
    #[must_use]
    pub const fn terminus() -> Self {
        Self(SelectExpr::Predicate(AtomPredicate::Terminus))
    }

    /// Atoms whose B-factor satisfies `comparison` against `threshold`.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidSelection`] when `threshold` is not finite.
    pub fn b_factor(comparison: PropertyComparison, threshold: f32) -> Result<Self, CoreError> {
        scalar(ScalarProperty::BFactor, comparison, threshold)
    }

    /// Atoms whose occupancy satisfies `comparison` against `threshold`.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidSelection`] when `threshold` is not finite.
    pub fn occupancy(comparison: PropertyComparison, threshold: f32) -> Result<Self, CoreError> {
        scalar(ScalarProperty::Occupancy, comparison, threshold)
    }

    /// Atoms within `distance` Å of `reference`.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidSelection`] when `distance` is negative or
    /// not finite.
    pub fn within(distance: f32, reference: Self) -> Result<Self, CoreError> {
        spatial(distance, reference, false)
    }

    /// Complete residues with any atom within `distance` Å of `reference`.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidSelection`] when `distance` is negative or
    /// not finite.
    pub fn residues_within(distance: f32, reference: Self) -> Result<Self, CoreError> {
        spatial(distance, reference, true)
    }

    /// Atoms outside `distance` Å of `reference`.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidSelection`] when `distance` is negative or
    /// not finite.
    pub fn beyond(distance: f32, reference: Self) -> Result<Self, CoreError> {
        Ok(spatial(distance, reference, false)?.negate())
    }

    /// Atoms inside a world-space sphere.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidSelection`] when the center or radius is
    /// non-finite, or the radius is negative.
    pub fn in_sphere(center: Vec3, radius: f32) -> Result<Self, CoreError> {
        if !center.is_finite() || !radius.is_finite() || radius < 0.0 {
            return Err(invalid("sphere center and radius must be finite and valid"));
        }
        Ok(Self(SelectExpr::InSphere { center, radius }))
    }

    /// Atoms inside a world-space axis-aligned box.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidSelection`] when bounds are non-finite or
    /// inverted.
    pub fn in_box(min: Vec3, max: Vec3) -> Result<Self, CoreError> {
        if !min.is_finite() || !max.is_finite() || min.x > max.x || min.y > max.y || min.z > max.z {
            return Err(invalid("box bounds must be finite and ordered"));
        }
        Ok(Self(SelectExpr::InBox { min, max }))
    }

    /// Logical intersection.
    #[must_use]
    pub fn and(self, other: Self) -> Self {
        Self(SelectExpr::And(Box::new(self.0), Box::new(other.0)))
    }

    /// Logical union.
    #[must_use]
    pub fn or(self, other: Self) -> Self {
        Self(SelectExpr::Or(Box::new(self.0), Box::new(other.0)))
    }

    /// Logical complement within each placed structure.
    #[must_use]
    pub fn negate(self) -> Self {
        Self(SelectExpr::Not(Box::new(self.0)))
    }
}

fn spatial(distance: f32, reference: Select, residues: bool) -> Result<Select, CoreError> {
    if !distance.is_finite() || distance < 0.0 {
        return Err(invalid("spatial distance must be finite and non-negative"));
    }
    Ok(Select(SelectExpr::Within {
        distance,
        reference: Box::new(reference.0),
        residues,
    }))
}

fn text_predicate(
    text: impl AsRef<str>,
    make: fn(Box<str>) -> AtomPredicate,
    reason: &'static str,
) -> Result<Select, CoreError> {
    let text = text.as_ref().trim();
    if text.is_empty() {
        return Err(invalid(reason));
    }
    Ok(Select(SelectExpr::Predicate(make(text.into()))))
}

fn scalar(
    property: ScalarProperty,
    comparison: PropertyComparison,
    threshold: f32,
) -> Result<Select, CoreError> {
    if !threshold.is_finite() {
        return Err(invalid("property threshold must be finite"));
    }
    Ok(Select(SelectExpr::Predicate(AtomPredicate::Scalar {
        property,
        comparison,
        threshold,
    })))
}

const fn invalid(reason: &'static str) -> CoreError {
    CoreError::InvalidSelection { reason }
}
