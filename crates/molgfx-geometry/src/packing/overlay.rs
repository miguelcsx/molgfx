//! Selection-scoped scheme overrides for the colour paths resolved on the CPU.
//!
//! Generated geometry such as a ribbon has no atom record for the GPU to
//! colour, so its vertex colours are resolved here, once per rebuild or
//! recolour. The override is the same one the GPU applies to atom records: a
//! per-atom class selects a scheme from the representation's overlay table.

use molgfx_core::{AtomProperty, ColorOverlay, ColorScheme};

/// One structure's class column together with the overlay that reads it.
#[derive(Clone, Copy, Debug)]
pub struct OverlayColumn<'a> {
    overlay: ColorOverlay,
    classes: &'a AtomProperty,
}

impl<'a> OverlayColumn<'a> {
    /// Pairs an overlay with the class column its handle names.
    #[must_use]
    pub const fn new(overlay: ColorOverlay, classes: &'a AtomProperty) -> Self {
        Self { overlay, classes }
    }

    /// The scheme that colours `atom`: its override, or `base`.
    ///
    /// Runtime is `O(1)` in atoms and `O(classes)` in the bounded table.
    #[must_use]
    pub fn scheme(&self, base: ColorScheme, atom: usize) -> ColorScheme {
        let Some(class) = self.classes.values().get(atom) else {
            return base;
        };
        match self.overlay.scheme_for(*class) {
            Some(scheme) => scheme,
            None => base,
        }
    }
}

#[cfg(test)]
#[path = "overlay_tests.rs"]
mod tests;
