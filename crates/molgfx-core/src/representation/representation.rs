//! Representations: how a selection is drawn.
//!
//! A representation is a view over the one scene model, never a second copy
//! of it. Multiple representations coexist on overlapping selections; their
//! draw order is the explicit `order` field, so layering is deterministic.
use crate::handle::{AtomPropertyHandle, SegmentationHandle, SelectionHandle, VolumeHandle};
use crate::{
    ClipSet, Material, PropertyAppearance, SegmentationStyle, SurfaceComponentPolicy,
    SurfaceScalarOverlay, TubeRadiusMapping,
};
use molgfx_math::Rgba8;
#[path = "color_scheme.rs"]
mod color_scheme;
pub use color_scheme::ColorColumns;
#[path = "kind_names.rs"]
mod kind_names;
#[path = "volume_types.rs"]
mod volume_types;
pub use volume_types::{
    MAX_VOLUME_TRANSFER_POINTS, VolumeRegion, VolumeRendering, VolumeSlice, VolumeStyle,
    VolumeTransferFunction, VolumeTransferPoint,
};
#[cfg(test)]
#[path = "representation_tests.rs"]
mod tests;
/// The catalogue of drawable forms.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RepresentationKind {
    /// Van der Waals spheres per atom.
    Spacefill,
    /// Small spheres joined by bond capsules.
    BallAndStick,
    /// Uniform-radius atom junctions and bond capsules.
    Licorice,
    /// Pixel-stable bond wires without atom junctions.
    Lines,
    /// Ribbon cartoon along the polymer trace.
    Cartoon,
    /// Thin smooth spline through polymer guide atoms.
    Trace,
    /// Round smooth tube through polymer guide atoms.
    Tube,
    /// A molecular surface.
    Surface,
    /// A density volume.
    Volume,
    /// A caller-supplied categorical label volume.
    Segmentation,
    /// One sphere per residue, enclosing that residue's atoms.
    Beads,
    /// Discrete secondary-structure solids along the polymer trace.
    Rocket,
    /// Ribbon along a glycan's glycosidic tree.
    Twister,
    /// Pucker-coloured ring bipyramids.
    PaperChain,
    /// One point per atom.
    Points,
}

/// Declarative parity presets built from the canonical representation paths.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum RepresentationPreset {
    /// Large atom spheres with bond capsules.
    Cpk,
    /// Uniform-radius atom junctions and bonds.
    Licorice,
    /// Filled ring polygons without a backbone tube.
    PaperChain,
    /// Solvent-accessible boundary shown as a pixel-stable dot lattice.
    DottedSolvent,
    /// Positive and negative isosurfaces of one signed scalar field.
    SignedIsosurface {
        /// Strictly negative level.
        negative_level: f32,
        /// Strictly positive level.
        positive_level: f32,
        /// Negative-lobe colour.
        negative_color: Rgba8,
        /// Positive-lobe colour.
        positive_color: Rgba8,
    },
}

/// Molecular boundaries whose geometry is never silently substituted.
#[repr(u32)]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum SurfaceKind {
    /// Boundary of the union of uninflated van der Waals atomic spheres.
    VanDerWaals = 0,
    /// Probe-center boundary around probe-inflated atomic radii.
    SolventAccessible = 1,
    /// Rolling-probe contact and reentrant molecular boundary.
    #[default]
    SolventExcluded = 2,
    /// Iso-density boundary of caller-controlled atom-centered Gaussians.
    Gaussian = 3,
}
/// Visual presentation of one molecular surface field.
#[repr(u32)]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum SurfaceStyle {
    /// Continuous opaque or translucent molecular boundary.
    #[default]
    Solid = 0,
    /// Pixel-stable triplanar contour lattice over the boundary.
    Contour = 1,
    /// Pixel-stable triplanar dot lattice over the boundary.
    Dots = 2,
    /// Translucent boundary with stronger pixel-stable contour lines.
    FilledContour = 3,
    /// Pixel-stable triangular wire lattice without a filled boundary.
    Mesh = 4,
    /// Rounded soft-union preview of an analytic solvent-accessible boundary.
    ///
    /// It rounds sphere-intersection cusps for inspection and is deliberately
    /// distinct from exact [`SurfaceStyle::Solid`] measurement geometry.
    SoftUnion = 5,
}
/// How atoms in a representation are colored.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
#[non_exhaustive]
pub enum ColorScheme {
    /// The classic per-element convention.
    #[default]
    ByElement,
    /// A categorical column (a chain, an entity, a residue name, a secondary
    /// structure class) coloured through a palette.
    ///
    /// The column is a scene property holding one whole-number category per
    /// atom; the palette cycles when categories outnumber its colours. An atom
    /// with no category (a non-finite value) keeps its element colour, which is
    /// how "colour only the carbons by chain" is expressed.
    ByCategory {
        /// Scene property column of categories.
        property: AtomPropertyHandle,
        /// Palette the categories index.
        palette: crate::CategoryPalette,
    },
    /// One arbitrary caller-supplied atom scalar through a reversible ramp.
    ByProperty {
        /// Scene property column.
        property: AtomPropertyHandle,
        /// Numeric-to-colour mapping emitted with a legend.
        ramp: crate::ScalarRamp,
        /// Explicit colour for missing values.
        missing: Rgba8,
    },
    /// A single color everywhere.
    Uniform(Rgba8),
}
/// The scene data a representation visualizes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RepresentationTarget {
    /// A molecular atom selection, expanded per placed structure.
    Selection(SelectionHandle),
    /// One caller-supplied scalar density grid.
    Volume(VolumeHandle),
    /// One caller-supplied categorical label grid.
    SegmentedVolume(SegmentationHandle),
}
/// Numeric parameters a caller may want to change per representation.
/// Defaults are the community-standard values.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct RepresentationParams {
    /// Scale applied to van der Waals radii (1.0 for spacefill, smaller for
    /// ball-and-stick spheres).
    pub radius_scale: f32,
    /// Bond capsule radius, Ångström.
    ///
    /// The default is the value the reference engines draw a stick with, so a
    /// caller who authors no radius gets the familiar picture.
    pub bond_radius: f32,
    /// Solvent probe radius for surfaces, Ångström; water by default.
    pub probe_radius: f32,
    /// Standard deviation of the atom-centred Gaussian density, Ångström.
    /// This is used only by [`SurfaceKind::Gaussian`].
    pub gaussian_sigma: f32,
    /// Level-set threshold for surfaces and volumes.
    pub isolevel: f32,
    /// Which physically distinct molecular boundary a surface renders.
    pub surface_kind: SurfaceKind,
    /// How the implicit surface field is presented.
    pub surface_style: SurfaceStyle,
    /// Connected-component policy for sampled molecular surface fields.
    pub surface_components: SurfaceComponentPolicy,
    /// Local-space spacing between contour lines or dots, Ångström.
    pub surface_pattern_spacing: f32,
    /// Blend span of the soft-union (blob) surface, Ångström.
    ///
    /// How far two atoms' surfaces round into each other: a larger span
    /// smooths a wider cusp away, a span near zero leaves the exact union.
    /// Used only by [`SurfaceStyle::SoftUnion`].
    pub blob_spread: f32,
    /// Contour half-width or dot radius in physical pixels.
    pub surface_pattern_width_pixels: f32,
    /// Cartoon strand and helix width, Ångström.
    pub ribbon_width: f32,
    /// Helix and sheet width divided by thickness.
    pub cartoon_aspect_ratio: f32,
    /// Arrow shoulder width relative to the strand body; zero disables arrows.
    pub cartoon_arrow_factor: f32,
    /// Draws a source-anchored N-to-C direction wedge at each polymer guide.
    pub cartoon_direction_wedges: bool,
    /// Presentation of real polymer gaps.
    pub gaps: crate::GapStyle,
    /// Cross-section of protein helices.
    pub cartoon_helix_profile: crate::CartoonProfile,
    /// Cross-section of nucleic-acid backbones.
    pub cartoon_nucleic_profile: crate::CartoonProfile,
    /// Radius of trace and tube spline cross-sections, Ångström.
    pub tube_radius: f32,
    /// Optional reversible property mapping for variable-radius tubes.
    pub tube_radius_mapping: TubeRadiusMapping,
    /// Diameter of a point marker in physical pixels.
    pub point_size_pixels: f32,
    /// Diameter of a bond wire in physical pixels.
    pub line_width_pixels: f32,
}
impl Default for RepresentationParams {
    fn default() -> Self {
        Self {
            radius_scale: 1.0,
            bond_radius: 0.25,
            probe_radius: 1.4,
            gaussian_sigma: 1.0,
            isolevel: 1.0,
            surface_kind: SurfaceKind::default(),
            surface_style: SurfaceStyle::default(),
            surface_components: SurfaceComponentPolicy::default(),
            surface_pattern_spacing: 1.5,
            blob_spread: 2.0,
            surface_pattern_width_pixels: 1.25,
            ribbon_width: 1.2,
            cartoon_aspect_ratio: 5.0,
            cartoon_arrow_factor: 1.5,
            cartoon_direction_wedges: false,
            gaps: crate::GapStyle::Hidden,
            cartoon_helix_profile: crate::CartoonProfile::Elliptical,
            cartoon_nucleic_profile: crate::CartoonProfile::Square,
            tube_radius: 0.3,
            tube_radius_mapping: TubeRadiusMapping::Constant,
            point_size_pixels: 3.0,
            line_width_pixels: 1.5,
        }
    }
}

pub(super) fn params_for_kind(kind: RepresentationKind) -> RepresentationParams {
    let mut params = RepresentationParams::default();
    match kind {
        RepresentationKind::BallAndStick => params.radius_scale = 0.25,
        RepresentationKind::Licorice => params.radius_scale = 1.0,
        RepresentationKind::Surface => params.isolevel = 0.0,
        RepresentationKind::Trace => params.tube_radius = 0.12,
        RepresentationKind::PaperChain => params.ribbon_width = 0.0,
        _ => {}
    }
    params
}
/// One drawable view over a selection.
#[derive(Clone, Debug)]
pub struct Representation {
    /// The molecular selection or scalar grid this view draws.
    pub target: RepresentationTarget,
    /// The drawn form.
    pub kind: RepresentationKind,
    /// Coloring rule.
    pub color: ColorScheme,
    /// Selection-scoped schemes that override `color` for some atoms.
    ///
    /// Shared with the structure's other representations; `None` colours
    /// every atom with `color`.
    pub color_overlay: Option<crate::ColorOverlay>,
    /// Optional reversible scalar-to-opacity-and-softness encoding.
    pub appearance: Option<PropertyAppearance>,
    /// Surface response.
    pub material: Material,
    /// Numeric knobs.
    pub params: RepresentationParams,
    /// Direct-volume transfer and sampling controls.
    pub volume: VolumeStyle,
    /// Categorical label styles and sampling controls.
    pub segmentation: SegmentationStyle,
    /// Optional caller-supplied scalar field sampled on a molecular surface.
    pub surface_scalar: Option<SurfaceScalarOverlay>,
    /// Per-representation world-space clipping and slabs.
    pub clipping: ClipSet,
    /// Whether the representation currently draws at all.
    pub visible: bool,
    /// Explicit draw order among overlapping representations; lower draws
    /// first.
    pub order: u16,
    /// Optional safe declarative visual behavior.
    pub visual: Option<crate::VisualStyle>,
}
impl Representation {
    /// A representation of the given kind over a selection, with defaults
    /// tuned per kind.
    #[must_use]
    pub fn new(target: RepresentationTarget, kind: RepresentationKind) -> Self {
        let mut material = Material::default();
        if kind == RepresentationKind::Surface {
            // A molecular boundary is a continuous solvent-facing envelope,
            // not a collection of polished atom impostors. A broader, weaker
            // dielectric lobe preserves curvature without making the field
            // read as wet plastic or exposing its sampling lattice.
            material.roughness = 0.62;
            material.specular = 0.22;
        }
        Self {
            target,
            kind,
            color: ColorScheme::default(),
            color_overlay: None,
            appearance: None,
            material,
            params: params_for_kind(kind),
            volume: VolumeStyle::default(),
            segmentation: SegmentationStyle::default(),
            surface_scalar: None,
            clipping: ClipSet::default(),
            visible: true,
            order: 0,
            visual: None,
        }
    }
    /// Molecular selection target, or `None` for a volume representation.
    #[must_use]
    pub const fn selection(&self) -> Option<SelectionHandle> {
        match self.target {
            RepresentationTarget::Selection(selection) => Some(selection),
            RepresentationTarget::Volume(_) | RepresentationTarget::SegmentedVolume(_) => None,
        }
    }
    /// Density-grid target, or `None` for a molecular representation.
    #[must_use]
    pub const fn volume_handle(&self) -> Option<VolumeHandle> {
        match self.target {
            RepresentationTarget::Selection(_) | RepresentationTarget::SegmentedVolume(_) => None,
            RepresentationTarget::Volume(volume) => Some(volume),
        }
    }
    /// Categorical label-grid target, or `None` for another representation.
    #[must_use]
    pub const fn segmentation_handle(&self) -> Option<SegmentationHandle> {
        match self.target {
            RepresentationTarget::SegmentedVolume(volume) => Some(volume),
            RepresentationTarget::Selection(_) | RepresentationTarget::Volume(_) => None,
        }
    }
    /// True when this representation must use order-independent alpha
    /// composition, including data-driven uncertainty softness.
    #[must_use]
    pub fn is_translucent(&self) -> bool {
        matches!(self.kind, RepresentationKind::Segmentation)
            || self.material.is_translucent()
            || self
                .appearance
                .is_some_and(PropertyAppearance::is_translucent)
            || self.visual.as_ref().is_some_and(|style| {
                style
                    .program()
                    .output_register(crate::VisualOutput::Opacity)
                    .is_some()
            })
    }
}
