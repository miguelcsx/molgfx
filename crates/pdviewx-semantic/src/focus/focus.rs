//! Focus-and-context distance bands.

use super::surface_zone::{SurfaceZoneScene, SurfaceZoneStyle};
use pdviewx_core::{
    ColorScheme, CoreError, RepresentationHandle, RepresentationKind, Scene, SelectionHandle,
    SurfaceKind, SurfaceStyle,
};
use pdviewx_math::Rgba8;

#[cfg(test)]
#[path = "focus_tests.rs"]
mod tests;

/// Graded context around a focus selection.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FocusBand {
    /// Interaction shell: full detail and emphasis.
    Near,
    /// Pocket context: muted intermediate detail.
    Mid,
    /// Orienting context: coarse and demoted.
    Far,
}

/// Polymer context geometry used around a focused molecular subject.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum FocusContext {
    /// Secondary-structure ribbon for conventional fold reading.
    Cartoon,
    /// Thin topology trace for the least occluding context.
    Trace,
    /// Smooth round backbone with restrained cinematic volume.
    #[default]
    Tube,
}

/// Spatial extent of the molecular boundary around a focused subject.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum FocusSurfaceExtent {
    /// Only atoms in the immediate interaction shell contribute to the field.
    #[default]
    Pocket,
    /// The complete non-solvent, non-focus structure contributes to one field.
    /// This is useful for clipped views that place a ligand inside a continuous
    /// protein cavity without stacking a second contextual surface.
    Structure,
}

impl FocusContext {
    const fn representation(self) -> RepresentationKind {
        match self {
            Self::Cartoon => RepresentationKind::Cartoon,
            Self::Trace => RepresentationKind::Trace,
            Self::Tube => RepresentationKind::Tube,
        }
    }
}

/// Monotonic distance thresholds in Ångström.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct DistanceBands {
    near: f32,
    mid: f32,
}

/// Invalid focus-band configuration.
#[derive(Clone, Copy, PartialEq, Eq, Debug, thiserror::Error)]
pub enum FocusError {
    /// Thresholds were non-finite, negative or not strictly increasing.
    #[error("focus distances must be finite and satisfy 0 <= near < mid")]
    InvalidDistances,
}

/// Editable selections and representations produced by semantic focus.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FocusView {
    /// Caller-supplied focus selection.
    pub focus: SelectionHandle,
    /// Complete residues in the interaction shell, excluding focused atoms.
    pub near: SelectionHandle,
    /// Atoms that bound the immediate pocket, excluding focused atoms.
    pub pocket: SelectionHandle,
    /// Complete residues in the orienting shell, excluding the near shell.
    pub mid: SelectionHandle,
    /// Everything beyond the orienting shell.
    pub context: SelectionHandle,
    /// Declared water entities inside the orienting shell.
    pub solvent: SelectionHandle,
    /// Full-detail focused atoms.
    pub focus_representation: RepresentationHandle,
    /// Detailed interaction-shell atoms.
    pub near_representation: RepresentationHandle,
    /// Simplified orienting-shell atoms.
    pub mid_representation: RepresentationHandle,
    /// Translucent pocket boundary.
    pub pocket_representation: RepresentationHandle,
    /// Demoted whole-structure orientation.
    pub context_representation: RepresentationHandle,
    /// Sparse caller-backed local solvent context.
    pub solvent_representation: RepresentationHandle,
}

/// Declarative focus composition parameters.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct FocusStyle {
    /// Spatial bands controlling semantic detail.
    pub bands: DistanceBands,
    /// Pocket-surface opacity.
    pub pocket_opacity: f32,
    /// Far-context cartoon opacity.
    pub context_opacity: f32,
    /// Boundary presentation for the independently editable pocket surface.
    pub pocket_presentation: SurfaceStyle,
    /// Whether the boundary is local to the pocket or continuous through the
    /// complete surrounding structure.
    pub surface_extent: FocusSurfaceExtent,
    /// Pocket surface colour.
    pub pocket_color: Rgba8,
    /// Far-context polymer geometry.
    pub context_geometry: FocusContext,
    /// Far-context colour.
    pub context_color: Rgba8,
    /// Local solvent opacity.
    pub solvent_opacity: f32,
    /// Local solvent colour.
    pub solvent_color: Rgba8,
}

impl Default for FocusStyle {
    fn default() -> Self {
        Self {
            bands: DistanceBands::default(),
            pocket_opacity: 0.34,
            context_opacity: 0.30,
            pocket_presentation: SurfaceStyle::Solid,
            surface_extent: FocusSurfaceExtent::Pocket,
            pocket_color: Rgba8::opaque(95, 126, 132),
            context_geometry: FocusContext::Tube,
            context_color: Rgba8::opaque(96, 116, 138),
            solvent_opacity: 0.42,
            solvent_color: Rgba8::opaque(72, 140, 156),
        }
    }
}

/// Semantic composition methods over the core scene model.
pub trait FocusScene {
    /// Builds the default focus-and-context composition.
    ///
    /// # Errors
    ///
    /// Returns a typed core error for a stale focus selection.
    fn focus(&mut self, selection: SelectionHandle) -> Result<FocusView, CoreError>;

    /// Builds a focus composition with caller-selected spatial and opacity
    /// policy. Generated handles remain independently editable.
    ///
    /// # Errors
    ///
    /// Returns a typed core error for a stale focus selection.
    fn focus_with(
        &mut self,
        selection: SelectionHandle,
        style: FocusStyle,
    ) -> Result<FocusView, CoreError>;
}

impl FocusScene for Scene {
    fn focus(&mut self, selection: SelectionHandle) -> Result<FocusView, CoreError> {
        self.focus_with(selection, FocusStyle::default())
    }

    fn focus_with(
        &mut self,
        selection: SelectionHandle,
        style: FocusStyle,
    ) -> Result<FocusView, CoreError> {
        if self.selection(selection).is_none()
            && !self
                .structures()
                .any(|(structure, _)| self.selection_for(selection, structure).is_some())
        {
            return Err(CoreError::StaleHandle);
        }
        let near_all = self.select_residues_within(selection, style.bands.near())?;
        let mid_all = self.select_residues_within(selection, style.bands.mid())?;
        let water = self.select_water();
        let solvent = self.intersect_selections(mid_all, water)?;
        let near_non_focus = self.difference_selections(near_all, selection)?;
        let near = self.difference_selections(near_non_focus, solvent)?;
        let mid_non_near = self.difference_selections(mid_all, near_all)?;
        let mid = self.difference_selections(mid_non_near, solvent)?;
        let context = self.complement_selection(mid_all)?;
        let non_focus = self.complement_selection(selection)?;
        let surface_source = self.difference_selections(non_focus, water)?;
        let surface_style = SurfaceZoneStyle {
            distance: style.bands.near(),
            kind: SurfaceKind::SolventExcluded,
            presentation: style.pocket_presentation,
            opacity: style.pocket_opacity,
            color: style.pocket_color,
        };
        let pocket_zone = match style.surface_extent {
            FocusSurfaceExtent::Pocket => {
                self.surface_zone_with(surface_source, selection, surface_style)?
            }
            FocusSurfaceExtent::Structure => {
                structure_surface(self, surface_source, surface_style)?
            }
        };
        let pocket = pocket_zone.selection;

        let context_representation =
            self.represent(context, style.context_geometry.representation())?;
        configure(
            self,
            context_representation,
            0,
            style.context_opacity,
            Some(style.context_color),
        );
        if let Some(context) = self.representation_mut(context_representation) {
            context.params.tube_radius = 0.16;
            context.material.roughness = 0.72;
            context.material.specular = 0.10;
        }
        let mid_representation = self.represent(mid, RepresentationKind::Points)?;
        configure(self, mid_representation, 1, 0.08, None);
        if let Some(mid) = self.representation_mut(mid_representation) {
            mid.params.point_size_pixels = 2.0;
        }
        let pocket_representation = pocket_zone.representation;
        if let Some(pocket) = self.representation_mut(pocket_representation) {
            pocket.order = 2;
            pocket.material.roughness = 0.84;
            pocket.material.specular = 0.06;
        }
        let near_representation = self.represent(near, RepresentationKind::BallAndStick)?;
        configure(self, near_representation, 3, 0.48, None);
        if let Some(near) = self.representation_mut(near_representation) {
            near.params.radius_scale = 0.11;
            near.params.bond_radius = 0.07;
        }
        let solvent_representation = self.represent(solvent, RepresentationKind::Spacefill)?;
        configure(
            self,
            solvent_representation,
            4,
            style.solvent_opacity,
            Some(style.solvent_color),
        );
        if let Some(solvent) = self.representation_mut(solvent_representation) {
            solvent.params.radius_scale = 0.24;
            solvent.material.roughness = 0.34;
            solvent.material.specular = 0.36;
        }
        let focus_representation = self.represent(selection, RepresentationKind::BallAndStick)?;
        configure(self, focus_representation, 5, 1.0, None);
        if let Some(focus) = self.representation_mut(focus_representation) {
            focus.material.roughness = 0.48;
            focus.material.specular = 0.30;
        }
        Ok(FocusView {
            focus: selection,
            near,
            pocket,
            mid,
            context,
            solvent,
            focus_representation,
            near_representation,
            mid_representation,
            pocket_representation,
            context_representation,
            solvent_representation,
        })
    }
}

fn structure_surface(
    scene: &mut Scene,
    selection: SelectionHandle,
    style: SurfaceZoneStyle,
) -> Result<crate::SurfaceZone, CoreError> {
    let representation = scene.represent(selection, RepresentationKind::Surface)?;
    if let Some(surface) = scene.representation_mut(representation) {
        surface.params.surface_kind = style.kind;
        surface.params.surface_style = style.presentation;
        surface.material.opacity = sanitize_opacity(style.opacity);
        surface.color = ColorScheme::Uniform(style.color);
    }
    Ok(crate::SurfaceZone {
        selection,
        representation,
    })
}

fn configure(
    scene: &mut Scene,
    handle: RepresentationHandle,
    order: u16,
    opacity: f32,
    color: Option<Rgba8>,
) {
    let Some(representation) = scene.representation_mut(handle) else {
        return;
    };
    representation.order = order;
    representation.material.opacity = if opacity.is_finite() {
        opacity.clamp(0.0, 1.0)
    } else {
        1.0
    };
    if let Some(color) = color {
        representation.color = ColorScheme::Uniform(color);
    }
}

fn sanitize_opacity(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        1.0
    }
}

impl Default for DistanceBands {
    fn default() -> Self {
        Self {
            near: 4.0,
            mid: 10.0,
        }
    }
}

impl DistanceBands {
    /// Builds validated distance bands.
    ///
    /// # Errors
    ///
    /// Returns [`FocusError::InvalidDistances`] for non-finite, negative or
    /// unordered thresholds.
    pub fn new(near: f32, mid: f32) -> Result<Self, FocusError> {
        if !near.is_finite() || !mid.is_finite() || near < 0.0 || near >= mid {
            return Err(FocusError::InvalidDistances);
        }
        Ok(Self { near, mid })
    }

    /// Classifies a non-negative distance; non-finite input is far context.
    #[must_use]
    pub fn classify(self, distance: f32) -> FocusBand {
        if distance < self.near {
            FocusBand::Near
        } else if distance < self.mid {
            FocusBand::Mid
        } else {
            FocusBand::Far
        }
    }

    /// Near-shell boundary in Ångström.
    #[must_use]
    pub const fn near(self) -> f32 {
        self.near
    }

    /// Mid-shell boundary in Ångström.
    #[must_use]
    pub const fn mid(self) -> f32 {
        self.mid
    }
}
