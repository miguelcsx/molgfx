//! Declarative selection-bounded molecular surface composition.
//!
//! The neighbourhood query reuses the scene BVH. A zone builds one dedicated
//! implicit field for the intersected atoms; unrelated representations and
//! their resident resources remain untouched.

use molgfx_core::{
    ColorScheme, CoreError, RepresentationHandle, RepresentationKind, Scene, SelectionHandle,
    SurfaceKind, SurfaceStyle,
};
use molgfx_math::Rgba8;

#[cfg(test)]
#[path = "surface_zone_tests.rs"]
mod tests;

/// Editable output of one surface-zone composition.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SurfaceZone {
    /// Atoms from the surface source within the requested neighbourhood.
    pub selection: SelectionHandle,
    /// Dedicated molecular-surface representation over [`Self::selection`].
    pub representation: RepresentationHandle,
}

/// Reusable presentation and boundary recipe for a surface zone.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct SurfaceZoneStyle {
    /// Spatial cutoff from the anchor selection, Ångström.
    pub distance: f32,
    /// Physical molecular boundary.
    pub kind: SurfaceKind,
    /// Solid, contour or dot presentation over the same field.
    pub presentation: SurfaceStyle,
    /// Zone opacity in `[0, 1]` after sanitization.
    pub opacity: f32,
    /// Uniform zone colour.
    pub color: Rgba8,
}

impl Default for SurfaceZoneStyle {
    fn default() -> Self {
        Self {
            distance: 5.0,
            kind: SurfaceKind::SolventExcluded,
            presentation: SurfaceStyle::Solid,
            opacity: 0.35,
            color: Rgba8::opaque(130, 184, 211),
        }
    }
}

/// Surface-zone composition over the core scene model.
pub trait SurfaceZoneScene {
    /// Renders the part of `surface` within 5 Å of `around` using the default
    /// translucent SES recipe.
    ///
    /// # Errors
    ///
    /// Returns a typed core error for stale selections or invalid distance.
    fn surface_zone(
        &mut self,
        surface: SelectionHandle,
        around: SelectionHandle,
    ) -> Result<SurfaceZone, CoreError>;

    /// Renders a caller-configured selection-bounded molecular surface.
    ///
    /// # Errors
    ///
    /// Returns a typed core error for stale selections or invalid distance.
    fn surface_zone_with(
        &mut self,
        surface: SelectionHandle,
        around: SelectionHandle,
        style: SurfaceZoneStyle,
    ) -> Result<SurfaceZone, CoreError>;

    /// Builds one surface after removing molecular graph components smaller
    /// than `minimum_atoms` from its source selection.
    ///
    /// # Errors
    ///
    /// Returns a typed core error for stale selections or a zero threshold.
    fn surface_components(
        &mut self,
        surface: SelectionHandle,
        minimum_atoms: u32,
    ) -> Result<SurfaceZone, CoreError>;
}

impl SurfaceZoneScene for Scene {
    fn surface_zone(
        &mut self,
        surface: SelectionHandle,
        around: SelectionHandle,
    ) -> Result<SurfaceZone, CoreError> {
        self.surface_zone_with(surface, around, SurfaceZoneStyle::default())
    }

    fn surface_zone_with(
        &mut self,
        surface: SelectionHandle,
        around: SelectionHandle,
        style: SurfaceZoneStyle,
    ) -> Result<SurfaceZone, CoreError> {
        let neighbourhood = self.select_within(around, style.distance)?;
        let selection = self.intersect_selections(surface, neighbourhood)?;
        let representation = self.represent(selection, RepresentationKind::Surface)?;
        if let Some(zone) = self.representation_mut(representation) {
            zone.params.surface_kind = style.kind;
            zone.params.surface_style = style.presentation;
            zone.material.opacity = sanitize_opacity(style.opacity);
            zone.color = ColorScheme::Uniform(style.color);
        }
        Ok(SurfaceZone {
            selection,
            representation,
        })
    }

    fn surface_components(
        &mut self,
        surface: SelectionHandle,
        minimum_atoms: u32,
    ) -> Result<SurfaceZone, CoreError> {
        let selection = self.select_molecular_components(surface, minimum_atoms)?;
        let representation = self.represent(selection, RepresentationKind::Surface)?;
        Ok(SurfaceZone {
            selection,
            representation,
        })
    }
}

fn sanitize_opacity(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        1.0
    }
}
