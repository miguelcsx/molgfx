//! Target-independent representation recipes for one-call scene authoring.

use super::{ColorScheme, Material, Representation, RepresentationKind, RepresentationParams};
use crate::{
    ClipSet, CoreError, PropertyAppearance, RepresentationTarget, SegmentationStyle,
    SurfaceComponentPolicy, SurfaceKind, SurfaceScalarOverlay, SurfaceStyle, TubeRadiusMapping,
    VisualStyle, VolumeStyle,
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
    visual: Option<VisualStyle>,
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
            visual: None,
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

    /// Uses one constant radius for a trace or tube spline.
    ///
    /// # Errors
    ///
    /// The radius must be finite and strictly positive, and this operation is
    /// only meaningful for [`RepresentationKind::Trace`] and
    /// [`RepresentationKind::Tube`].
    pub fn tube_radius(mut self, radius: f32) -> Result<Self, CoreError> {
        self.validate_tube_kind()?;
        if !radius.is_finite() || radius <= 0.0 {
            return Err(CoreError::InvalidProperty {
                reason: "tube radius must be finite and strictly positive",
            });
        }
        self.params.tube_radius = radius;
        self.params.tube_radius_mapping = TubeRadiusMapping::Constant;
        Ok(self)
    }

    /// Maps backbone B factors to a variable-radius putty tube on the GPU.
    ///
    /// The source values remain attached to guide atoms; changing this mapping
    /// does not rebuild spline geometry or copy the coordinate column.
    ///
    /// # Errors
    ///
    /// Returns a typed validation error for a non-tube representation or an
    /// invalid domain/radius pair.
    pub fn putty_b_factor(mut self, domain: [f32; 2], radii: [f32; 2]) -> Result<Self, CoreError> {
        self.validate_tube_kind()?;
        self.params.tube_radius_mapping = TubeRadiusMapping::b_factor(domain, radii)?;
        Ok(self)
    }

    fn validate_tube_kind(&self) -> Result<(), CoreError> {
        if matches!(
            self.kind,
            RepresentationKind::Trace | RepresentationKind::Tube
        ) {
            return Ok(());
        }
        Err(CoreError::InvalidProperty {
            reason: "tube radius controls require a trace or tube representation",
        })
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

    /// Filters small disconnected components from the sampled surface field.
    ///
    /// The policy is evaluated on GPU-resident working-set voxels. It does not
    /// enumerate the logical dataset or materialize a triangle surface.
    ///
    /// # Errors
    ///
    /// Returns a typed property error when applied to a non-surface recipe.
    pub fn hide_small_disconnected_components(
        mut self,
        policy: SurfaceComponentPolicy,
    ) -> Result<Self, CoreError> {
        if self.kind != RepresentationKind::Surface {
            return Err(CoreError::InvalidProperty {
                reason: "surface component filtering requires a surface representation",
            });
        }
        self.params.surface_components = policy;
        Ok(self)
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

    /// Applies one safe declarative visual program.
    #[must_use]
    pub fn visual(mut self, visual: VisualStyle) -> Self {
        self.visual = Some(visual);
        self
    }

    pub(crate) fn visual_style(&self) -> Option<&VisualStyle> {
        self.visual.as_ref()
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
        representation.visual = self.visual;
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

    /// B-factor-driven variable-radius tube with validated endpoint radii.
    ///
    /// # Errors
    ///
    /// The domain must increase and radii must be finite, positive and
    /// distinct.
    pub fn putty(domain: [f32; 2], radii: [f32; 2]) -> Result<RepresentationConfig, CoreError> {
        Self::tube().putty_b_factor(domain, radii)
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

    /// Pucker-coloured ring bipyramids.
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
