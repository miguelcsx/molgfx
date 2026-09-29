//! CPU colour resolution for geometry with no atom record to colour on the GPU.
//!
//! A ribbon or a residue bead is generated geometry, so its colours are baked
//! into the mesh here, once per rebuild or recolour. The function is the same
//! one the GPU applies to atom records, and both read the same palettes and
//! ramps from `molgfx-core`, so the two paths cannot disagree.
//!
//! Every colour input is a scene property column, looked up through the scene
//! by handle: a continuous ramp, a categorical palette, and a selection-scoped
//! overlay all resolve the same way, in `O(1)` per atom.

use molgfx_core::{
    AtomProperty, AtomPropertyHandle, ColorOverlay, ColorScheme, Scene, StructureHandle,
};
use molgfx_math::Rgba8;

/// The scene and structure whose property columns colour generated geometry.
#[derive(Clone, Copy)]
pub struct ColorContext<'a> {
    scene: &'a Scene,
    structure: StructureHandle,
}

impl std::fmt::Debug for ColorContext<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ColorContext")
            .field("structure", &self.structure)
            .finish_non_exhaustive()
    }
}

impl<'a> ColorContext<'a> {
    /// Colours atoms of `structure` from `scene`'s property columns.
    #[must_use]
    pub const fn new(scene: &'a Scene, structure: StructureHandle) -> Self {
        Self { scene, structure }
    }

    /// The column `handle` names, when it belongs to this structure.
    #[must_use]
    pub fn column(&self, handle: AtomPropertyHandle) -> Option<&'a AtomProperty> {
        self.scene.property_for_structure(handle, self.structure)
    }

    /// The scheme that colours `atom`: its overlay override, or `base`.
    ///
    /// Runtime is `O(1)` in atoms and `O(classes)` in the bounded table.
    #[must_use]
    pub fn scheme(
        &self,
        base: ColorScheme,
        overlay: Option<ColorOverlay>,
        atom: usize,
    ) -> ColorScheme {
        let Some(overlay) = overlay else {
            return base;
        };
        let class = self
            .column(overlay.classes())
            .and_then(|classes| classes.values().get(atom));
        match class.and_then(|class| overlay.scheme_for(*class)) {
            Some(scheme) => scheme,
            None => base,
        }
    }

    /// The colour `scheme` gives `atom`, whose own element colour is `element`.
    ///
    /// A scheme whose column is absent, or whose value for this atom is not a
    /// category, falls back to the element colour rather than to an arbitrary
    /// palette entry.
    #[must_use]
    pub fn color(&self, scheme: ColorScheme, element: Rgba8, atom: usize) -> Rgba8 {
        match scheme {
            ColorScheme::Uniform(color) => color,
            ColorScheme::ByCategory { property, palette } => {
                let category = self
                    .column(property)
                    .and_then(|column| column.values().get(atom))
                    .and_then(|category| palette.color(*category));
                let Some(color) = category else {
                    return element;
                };
                color
            }
            ColorScheme::ByProperty {
                property,
                ramp,
                missing,
            } => match self
                .column(property)
                .and_then(|column| column.values().get(atom))
            {
                Some(value) => ramp.sample(*value, missing),
                None => missing,
            },
            _ => element,
        }
    }
}

#[cfg(test)]
#[path = "color_tests.rs"]
mod tests;
