//! Selection-scoped colour overrides layered over a representation's scheme.
//!
//! A representation colours everything it draws with one [`ColorScheme`]. An
//! overlay lets a subset of its atoms take a different scheme without splitting
//! the representation: every atom of the structure carries a small class in a
//! scalar property column, class zero meaning "no override", and the overlay
//! maps each non-zero class to one scheme from a fixed-size table.
//!
//! The column belongs to the structure, not to a representation, so every
//! representation of that structure shares it. A frame reads one class per
//! drawn atom and indexes the table; no rule, query or string is evaluated
//! while drawing, and the table is a bounded block of uniform data.

use crate::{AtomPropertyHandle, ColorScheme, CoreError};

/// The most distinct overriding schemes one overlay can name.
///
/// The bound makes the table a fixed-size uniform block, so an overlay never
/// changes the layout a shader reads.
pub const MAX_COLOR_OVERLAY_CLASSES: usize = 15;

/// Per-atom colour-scheme overrides shared by a structure's representations.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ColorOverlay {
    classes: AtomPropertyHandle,
    schemes: [ColorScheme; MAX_COLOR_OVERLAY_CLASSES],
    len: u8,
}

impl ColorOverlay {
    /// An overlay whose class `k` (one-based) selects `schemes[k - 1]`.
    ///
    /// `classes` is a scalar atom property of the drawn structure holding one
    /// class per atom: zero, or a non-finite value, leaves the atom's own
    /// scheme in place.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidProperty`] for an empty table, more than
    /// [`MAX_COLOR_OVERLAY_CLASSES`] schemes, or a property-driven scheme,
    /// which needs a column of its own and cannot share this one.
    pub fn new(classes: AtomPropertyHandle, schemes: &[ColorScheme]) -> Result<Self, CoreError> {
        if schemes.is_empty() || schemes.len() > MAX_COLOR_OVERLAY_CLASSES {
            return Err(CoreError::InvalidProperty {
                reason: "a colour overlay names between one and fifteen schemes",
            });
        }
        if schemes
            .iter()
            .any(|scheme| matches!(scheme, ColorScheme::ByProperty { .. }))
        {
            return Err(CoreError::InvalidProperty {
                reason: "a colour overlay cannot use a property-driven scheme",
            });
        }
        let mut table = [ColorScheme::ByElement; MAX_COLOR_OVERLAY_CLASSES];
        table[..schemes.len()].copy_from_slice(schemes);
        let Ok(len) = u8::try_from(schemes.len()) else {
            return Err(CoreError::InvalidProperty {
                reason: "a colour overlay names between one and fifteen schemes",
            });
        };
        Ok(Self {
            classes,
            schemes: table,
            len,
        })
    }

    /// The per-atom class column.
    #[must_use]
    pub const fn classes(&self) -> AtomPropertyHandle {
        self.classes
    }

    /// The overriding schemes, class one first.
    #[must_use]
    pub fn schemes(&self) -> &[ColorScheme] {
        &self.schemes[..usize::from(self.len)]
    }

    /// The scheme one class value selects, or `None` to keep the base scheme.
    ///
    /// The class column stores whole numbers as scalars; anything that is not
    /// a positive whole number within the table selects nothing.
    #[must_use]
    pub fn scheme_for(&self, class: f32) -> Option<ColorScheme> {
        if !class.is_finite() || class < 1.0 || class.fract() != 0.0 {
            return None;
        }
        // Whole numbers up to the table size convert exactly, so comparing
        // their bit patterns is an exact equality test, not a tolerance.
        (1..=self.len)
            .zip(self.schemes().iter().copied())
            .find_map(|(ordinal, scheme)| {
                (f32::from(ordinal).to_bits() == class.to_bits()).then_some(scheme)
            })
    }
}

#[cfg(test)]
#[path = "color_overlay_tests.rs"]
mod tests;
