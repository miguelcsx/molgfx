//! Representations: how a selection is drawn.
//!
//! A representation is a view over the one scene model, never a second copy
//! of it. Multiple representations coexist on overlapping selections; their
//! draw order is the explicit `order` field, so layering is deterministic.

use crate::handle::SelectionHandle;
use pdviewx_math::Rgba8;

/// The catalogue of drawable forms.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[non_exhaustive]
pub enum RepresentationKind {
    /// Van der Waals spheres per atom.
    Spacefill,
    /// Small spheres joined by bond capsules.
    BallAndStick,
    /// Ribbon cartoon along the polymer trace.
    Cartoon,
    /// A molecular surface.
    Surface,
    /// A density volume.
    Volume,
    /// One point per atom.
    Points,
}

/// How atoms in a representation are colored.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
#[non_exhaustive]
pub enum ColorScheme {
    /// The classic per-element convention.
    #[default]
    ByElement,
    /// One color per chain.
    ByChain,
    /// A single color everywhere.
    Uniform(Rgba8),
}

/// Surface response of a drawn representation.
///
/// The science-driven material system arrives with the semantic layer; the
/// plain variant carries the fields the base lighting model already needs.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Material {
    /// Overall opacity in [0, 1]; below 1 the representation draws in the
    /// translucent pass.
    pub opacity: f32,
    /// Specular strength in [0, 1]; kept low, molecules are not chrome.
    pub specular: f32,
}

impl Default for Material {
    fn default() -> Self {
        Self {
            opacity: 1.0,
            specular: 0.25,
        }
    }
}

/// Numeric parameters a scientist may want to change per representation.
/// Defaults are the community-standard values.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct RepresentationParams {
    /// Scale applied to van der Waals radii (1.0 for spacefill, smaller for
    /// ball-and-stick spheres).
    pub radius_scale: f32,
    /// Bond capsule radius, Ångström.
    pub bond_radius: f32,
    /// Solvent probe radius for surfaces, Ångström; water by default.
    pub probe_radius: f32,
    /// Level-set threshold for surfaces and volumes.
    pub isolevel: f32,
    /// Cartoon strand and helix width, Ångström.
    pub ribbon_width: f32,
}

impl Default for RepresentationParams {
    fn default() -> Self {
        Self {
            radius_scale: 1.0,
            bond_radius: 0.18,
            probe_radius: 1.4,
            isolevel: 1.0,
            ribbon_width: 1.2,
        }
    }
}

/// One drawable view over a selection.
#[derive(Clone, Debug)]
pub struct Representation {
    /// The rows this representation draws.
    pub selection: SelectionHandle,
    /// The drawn form.
    pub kind: RepresentationKind,
    /// Coloring rule.
    pub color: ColorScheme,
    /// Surface response.
    pub material: Material,
    /// Numeric knobs.
    pub params: RepresentationParams,
    /// Whether the representation currently draws at all.
    pub visible: bool,
    /// Explicit draw order among overlapping representations; lower draws
    /// first.
    pub order: u16,
}

impl Representation {
    /// A representation of the given kind over a selection, with defaults
    /// tuned per kind.
    #[must_use]
    pub fn new(selection: SelectionHandle, kind: RepresentationKind) -> Self {
        let mut params = RepresentationParams::default();
        if kind == RepresentationKind::BallAndStick {
            // Classic ball-and-stick proportions: small spheres, thin bonds.
            params.radius_scale = 0.25;
        }
        Self {
            selection,
            kind,
            color: ColorScheme::default(),
            material: Material::default(),
            params,
            visible: true,
            order: 0,
        }
    }
}
