//! Target-independent representation recipes for one-call scene authoring.

use super::{ColorScheme, Material, Representation, RepresentationKind, RepresentationParams};
use crate::{
    ClipSet, PropertyAppearance, RepresentationTarget, SegmentationStyle, SurfaceKind,
    SurfaceScalarOverlay, SurfaceStyle, VolumeStyle,
};

/// A reusable, target-independent representation recipe.
///
/// Recipes keep selection compilation and GPU batching inside [`crate::Scene`]:
/// callers describe a view once instead of creating or mutating draw objects.
#[derive(Clone, PartialEq, Debug)]
pub struct RepresentationConfig {
    pub(crate) kind: RepresentationKind,
    pub(crate) color: ColorScheme,
    pub(crate) material: Material,
    pub(crate) params: RepresentationParams,
    pub(crate) clipping: ClipSet,
    appearance: Option<PropertyAppearance>,
    volume: VolumeStyle,
    segmentation: SegmentationStyle,
    surface_scalar: Option<SurfaceScalarOverlay>,
    visible: bool,
    order: u16,
}

impl RepresentationConfig {
    /// Starts a recipe with the tuned defaults for `kind`.
    #[must_use]
    pub fn new(kind: RepresentationKind) -> Self {
        Self {
            kind,
            color: ColorScheme::default(),
            material: Material::default(),
            params: super::kinds::params_for_kind(kind),
            clipping: ClipSet::default(),
            appearance: None,
            volume: VolumeStyle::default(),
            segmentation: SegmentationStyle::default(),
            surface_scalar: None,
            visible: true,
            order: 0,
        }
    }

    /// Representation form selected by this recipe.
    #[must_use]
    pub const fn kind(&self) -> RepresentationKind {
        self.kind
    }

    /// Applies one GPU-resolved color rule.
    #[must_use]
    pub const fn color(mut self, color: ColorScheme) -> Self {
        self.color = color;
        self
    }

    /// Applies one material to the whole batched representation.
    #[must_use]
    pub const fn material(mut self, material: Material) -> Self {
        self.material = material;
        self
    }

    /// Applies world-space clipping without changing source geometry.
    #[must_use]
    pub const fn clipping(mut self, clipping: ClipSet) -> Self {
        self.clipping = clipping;
        self
    }

    /// Scales atomic radii in the GPU representation.
    #[must_use]
    pub const fn radius_scale(mut self, scale: f32) -> Self {
        self.params.radius_scale = scale;
        self
    }

    /// Sets the bond capsule radius in Angstrom.
    #[must_use]
    pub const fn bond_radius(mut self, radius: f32) -> Self {
        self.params.bond_radius = radius;
        self
    }

    /// Sets the scalar level for molecular or volume isosurfaces.
    #[must_use]
    pub const fn isolevel(mut self, level: f32) -> Self {
        self.params.isolevel = level;
        self
    }

    /// Selects molecular-surface construction and presentation in one step.
    #[must_use]
    pub const fn surface(mut self, kind: SurfaceKind, style: SurfaceStyle) -> Self {
        self.params.surface_kind = kind;
        self.params.surface_style = style;
        self
    }

    /// Applies reversible scalar-to-opacity and softness encoding.
    #[must_use]
    pub const fn appearance(mut self, appearance: PropertyAppearance) -> Self {
        self.appearance = Some(appearance);
        self
    }

    /// Applies direct-volume, isosurface, medium, slice, or liquid controls.
    #[must_use]
    pub const fn volume_style(mut self, style: VolumeStyle) -> Self {
        self.volume = style;
        self
    }

    /// Applies categorical-volume label and sampling controls.
    #[must_use]
    pub fn segmentation_style(mut self, style: SegmentationStyle) -> Self {
        self.segmentation = style;
        self
    }

    /// Samples one caller scalar field on a molecular surface.
    #[must_use]
    pub const fn surface_scalar(mut self, overlay: SurfaceScalarOverlay) -> Self {
        self.surface_scalar = Some(overlay);
        self
    }

    /// Sets stable draw ordering for overlapping views.
    #[must_use]
    pub const fn order(mut self, order: u16) -> Self {
        self.order = order;
        self
    }

    pub(crate) fn bind(self, target: RepresentationTarget) -> Representation {
        let mut representation = Representation::new(target, self.kind);
        representation.color = self.color;
        representation.material = self.material;
        representation.params = self.params;
        representation.clipping = self.clipping;
        representation.appearance = self.appearance;
        representation.volume = self.volume;
        representation.segmentation = self.segmentation;
        representation.surface_scalar = self.surface_scalar;
        representation.visible = self.visible;
        representation.order = self.order;
        representation
    }
}

impl From<RepresentationKind> for RepresentationConfig {
    fn from(kind: RepresentationKind) -> Self {
        Self::new(kind)
    }
}

impl Representation {
    /// Van der Waals atom spheres.
    #[must_use]
    pub fn spacefill() -> RepresentationConfig {
        RepresentationKind::Spacefill.into()
    }

    /// Atom spheres joined by bond capsules.
    #[must_use]
    pub fn ball_and_stick() -> RepresentationConfig {
        RepresentationKind::BallAndStick.into()
    }

    /// Pixel-stable round bond wires.
    #[must_use]
    pub fn lines() -> RepresentationConfig {
        RepresentationKind::Lines.into()
    }

    /// Uniform-radius atom junctions and bond capsules.
    #[must_use]
    pub fn licorice() -> RepresentationConfig {
        RepresentationKind::Licorice.into()
    }

    /// Protein secondary-structure ribbon.
    #[must_use]
    pub fn cartoon() -> RepresentationConfig {
        RepresentationKind::Cartoon.into()
    }

    /// Thin polymer spline.
    #[must_use]
    pub fn trace() -> RepresentationConfig {
        RepresentationKind::Trace.into()
    }

    /// Round polymer tube.
    #[must_use]
    pub fn tube() -> RepresentationConfig {
        RepresentationKind::Tube.into()
    }

    /// Molecular implicit surface.
    #[must_use]
    pub fn surface() -> RepresentationConfig {
        RepresentationKind::Surface.into()
    }

    /// One analytic point per selected atom.
    #[must_use]
    pub fn points() -> RepresentationConfig {
        RepresentationKind::Points.into()
    }

    /// One enclosing sphere per residue.
    #[must_use]
    pub fn beads() -> RepresentationConfig {
        RepresentationKind::Beads.into()
    }

    /// Solid secondary-structure glyphs.
    #[must_use]
    pub fn rocket() -> RepresentationConfig {
        RepresentationKind::Rocket.into()
    }

    /// Glycan ribbon tree.
    #[must_use]
    pub fn twister() -> RepresentationConfig {
        RepresentationKind::Twister.into()
    }

    /// Filled nucleotide or carbohydrate rings.
    #[must_use]
    pub fn paper_chain() -> RepresentationConfig {
        RepresentationKind::PaperChain.into()
    }

    /// Direct scalar-volume rendering recipe.
    #[must_use]
    pub fn volume() -> RepresentationConfig {
        RepresentationKind::Volume.into()
    }

    /// Categorical label-volume recipe.
    #[must_use]
    pub fn segmentation() -> RepresentationConfig {
        RepresentationKind::Segmentation.into()
    }
}
