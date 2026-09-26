//! The declarative appearance rule value.

use crate::color::ColorSpec;
use crate::id::StructureId;
use crate::representation::Selection;
use serde::{Deserialize, Serialize};

/// The most distinct colourings one structure's rules may resolve to.
///
/// Rules that share a colour share a class, so this bounds distinct colours,
/// not rules. The bound keeps the per-representation colour table a fixed-size
/// uniform block: a frame never pays for a larger table than it declares.
pub const MAX_APPEARANCE_CLASSES: usize = molgfx_core::MAX_COLOR_OVERLAY_CLASSES;

/// Colours the atoms of one structure that match a query.
///
/// The colour overrides the base colour of every representation drawing those
/// atoms. Only colourings that are a function of the atom itself are accepted:
/// element, chain, residue, secondary structure, or one uniform colour. A
/// property colour needs a whole representation's legend and domain, so it is
/// set on a representation instead.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct AppearanceRuleSpec {
    /// Structure whose atoms the rule colours.
    pub structure: StructureId,
    /// `MolFrame` query selecting the coloured atoms.
    pub target: Selection,
    /// Colouring applied to the selected atoms.
    pub color: ColorSpec,
}

impl AppearanceRuleSpec {
    /// A rule colouring `target` atoms of `structure`.
    #[must_use]
    pub fn new(structure: StructureId, target: impl Into<Selection>, color: ColorSpec) -> Self {
        Self {
            structure,
            target: target.into(),
            color,
        }
    }

    pub(crate) fn validate(&self) -> Result<(), crate::Error> {
        self.color.validate()?;
        if matches!(self.color, ColorSpec::Property { .. }) {
            return Err(crate::Error::InvalidSpec(
                "an appearance rule cannot use a property colour; set it on a representation"
                    .to_owned(),
            ));
        }
        let _ = self.target.fingerprint()?;
        Ok(())
    }

    /// Deterministic explanation of the rule.
    ///
    /// # Errors
    ///
    /// Returns an invalid-specification error when the query cannot compile.
    pub fn explain(&self) -> Result<String, crate::Error> {
        Ok(format!(
            "AppearanceRule\nstructure: {}\ntarget: {}\nhash: {}\ncolor: {:?}",
            self.structure.get(),
            self.target.source(),
            self.target.stable_hash()?,
            self.color
        ))
    }
}
