//! Representations: how a selection is drawn.
//!
//! A representation is a view over the one scene model, never a second copy
//! of it. Multiple representations coexist on overlapping selections; their
//! draw order is the explicit `order` field, so layering is deterministic.
use crate::handle::{AtomPropertyHandle, SegmentationHandle, SelectionHandle, VolumeHandle};
use crate::{
    ClipSet, CoreError, Material, PropertyAppearance, SegmentationStyle, SurfaceScalarOverlay,
    TubeRadiusMapping,
};
use pdviewx_math::Rgba8;

#[path = "kind_names.rs"]
mod kind_names;

#[path = "color_scheme.rs"]
mod color_scheme;
#[cfg(test)]
#[path = "representation_tests.rs"]
mod tests;

/// The catalogue of drawable forms.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[non_exhaustive]
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
    /// One filled ring polygon per selected nucleotide or carbohydrate ring.
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

/// Molecular boundary construction. The two variants have different
/// geometry and are never silently substituted for one another.
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
    /// Stable CVD-safe categorical colour by source residue row.
    ByResidue,
    /// Helix, sheet and coil receive distinct CVD-safe colours.
    BySecondaryStructure,
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
/// Maximum transfer points kept in one compact volume uniform.
pub const MAX_VOLUME_TRANSFER_POINTS: usize = 8;
/// One scalar-to-color-and-opacity transfer control point.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct VolumeTransferPoint {
    /// Scalar value at this point.
    pub value: f32,
    /// Reversible scientific color.
    pub color: Rgba8,
    /// Optical response in [0, 1].
    pub opacity: f32,
}
impl VolumeTransferPoint {
    /// Creates one point.
    #[must_use]
    pub const fn new(value: f32, color: Rgba8, opacity: f32) -> Self {
        Self {
            value,
            color,
            opacity,
        }
    }
}
/// Ordered, fixed-capacity direct-volume transfer function.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct VolumeTransferFunction {
    points: [VolumeTransferPoint; MAX_VOLUME_TRANSFER_POINTS],
    len: u8,
}
impl VolumeTransferFunction {
    /// Validates two to eight strictly ordered finite control points.
    ///
    /// # Errors
    ///
    /// Values must increase strictly and opacities must be finite in [0, 1].
    pub fn new(points: &[VolumeTransferPoint]) -> Result<Self, CoreError> {
        if !(2..=MAX_VOLUME_TRANSFER_POINTS).contains(&points.len()) {
            return Err(invalid_transfer(
                "transfer function requires two to eight points",
            ));
        }
        if points.iter().any(|point| {
            !point.value.is_finite()
                || !point.opacity.is_finite()
                || !(0.0..=1.0).contains(&point.opacity)
        }) || points.windows(2).any(|pair| pair[0].value >= pair[1].value)
        {
            return Err(invalid_transfer(
                "transfer values must increase and opacities must be finite",
            ));
        }
        let mut transfer = Self::default();
        transfer.points[..points.len()].copy_from_slice(points);
        transfer.len = u8::try_from(points.len()).map_or(2, |len| len);
        Ok(transfer)
    }
    /// Linear transparent-to-opaque map over one scalar interval.
    #[must_use]
    pub fn linear(range: [f32; 2], low: Rgba8, high: Rgba8) -> Self {
        let high_value = if range[1].is_finite() && range[1] > range[0] {
            range[1]
        } else {
            range[0] + 1.0
        };
        Self {
            points: [
                VolumeTransferPoint::new(range[0], low, 0.0),
                VolumeTransferPoint::new(high_value, high, 1.0),
                VolumeTransferPoint::new(0.0, Rgba8::WHITE, 0.0),
                VolumeTransferPoint::new(0.0, Rgba8::WHITE, 0.0),
                VolumeTransferPoint::new(0.0, Rgba8::WHITE, 0.0),
                VolumeTransferPoint::new(0.0, Rgba8::WHITE, 0.0),
                VolumeTransferPoint::new(0.0, Rgba8::WHITE, 0.0),
                VolumeTransferPoint::new(0.0, Rgba8::WHITE, 0.0),
            ],
            len: 2,
        }
    }
    /// Active points in scalar order.
    #[must_use]
    pub fn points(&self) -> &[VolumeTransferPoint] {
        &self.points[..usize::from(self.len)]
    }
}
impl Default for VolumeTransferFunction {
    fn default() -> Self {
        Self::linear(
            [0.0, 1.0],
            Rgba8::opaque(68, 1, 84),
            Rgba8::opaque(253, 231, 37),
        )
    }
}
/// Transfer function and sampling controls for a density volume.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct VolumeStyle {
    /// Direct integration or one lit scalar isosurface over the same grid.
    pub rendering: VolumeRendering,
    /// Piecewise-linear scalar, colour and opacity mapping.
    pub transfer: VolumeTransferFunction,
    /// Optical-density multiplier applied after material opacity.
    pub opacity_scale: f32,
    /// Ray step relative to the smallest voxel axis; lower is more accurate.
    pub step_scale: f32,
    /// World-space plane sampled by [`VolumeRendering::Slice`].
    pub slice: Option<VolumeSlice>,
    /// Optional half-open voxel region rendered from the resident grid.
    pub region: Option<VolumeRegion>,
}
impl Default for VolumeStyle {
    fn default() -> Self {
        Self {
            rendering: VolumeRendering::Direct,
            transfer: VolumeTransferFunction::default(),
            opacity_scale: 2.0,
            step_scale: 0.65,
            slice: None,
            region: None,
        }
    }
}
/// A validated half-open voxel region `[minimum, maximum)`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct VolumeRegion {
    minimum: [u32; 3],
    maximum: [u32; 3],
}
impl VolumeRegion {
    /// Validates a crop against its source grid dimensions.
    ///
    /// # Errors
    ///
    /// Every axis must be non-empty and remain inside `dimensions`.
    pub fn new(
        minimum: [u32; 3],
        maximum: [u32; 3],
        dimensions: [u32; 3],
    ) -> Result<Self, CoreError> {
        if (0..3).any(|axis| minimum[axis] >= maximum[axis] || maximum[axis] > dimensions[axis]) {
            return Err(invalid_transfer(
                "volume region must be non-empty and inside the source grid",
            ));
        }
        Ok(Self { minimum, maximum })
    }
    /// Inclusive minimum voxel index.
    #[must_use]
    pub const fn minimum(self) -> [u32; 3] {
        self.minimum
    }
    /// Exclusive maximum voxel index.
    #[must_use]
    pub const fn maximum(self) -> [u32; 3] {
        self.maximum
    }
}
/// One arbitrary world-space plane through a caller scalar grid.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct VolumeSlice {
    /// Plane equation; unlike clipping, this is the surface that is drawn.
    pub plane: crate::ClipPlane,
}
impl VolumeSlice {
    /// Creates a slice from a validated world-space plane.
    #[must_use]
    pub const fn new(plane: crate::ClipPlane) -> Self {
        Self { plane }
    }
}
/// Rendering algorithm over one caller-supplied scalar grid.
#[repr(u32)]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum VolumeRendering {
    /// Front-to-back optical integration through the scalar field.
    #[default]
    Direct = 0,
    /// Lit implicit boundary at [`RepresentationParams::isolevel`].
    Isosurface = 1,
    /// Single-scattered participating medium from caller-supplied density.
    Medium = 2,
    /// Transfer-mapped scalar values on one arbitrary world-space plane.
    Slice = 3,
    /// Screen-space liquid-like boundary over caller-provided scalar density.
    /// No advection or fluid simulation is performed by the renderer.
    LiquidSurface = 4,
}
const fn invalid_transfer(reason: &'static str) -> CoreError {
    CoreError::InvalidVolume { reason }
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
    /// Standard deviation of the atom-centred Gaussian density, Ångström.
    /// This is used only by [`SurfaceKind::Gaussian`].
    pub gaussian_sigma: f32,
    /// Level-set threshold for surfaces and volumes.
    pub isolevel: f32,
    /// Which physically distinct molecular boundary a surface renders.
    pub surface_kind: SurfaceKind,
    /// How the implicit surface field is presented.
    pub surface_style: SurfaceStyle,
    /// Local-space spacing between contour lines or dots, Ångström.
    pub surface_pattern_spacing: f32,
    /// Contour half-width or dot radius in physical pixels.
    pub surface_pattern_width_pixels: f32,
    /// Cartoon strand and helix width, Ångström.
    pub ribbon_width: f32,
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
            bond_radius: 0.18,
            probe_radius: 1.4,
            gaussian_sigma: 1.0,
            isolevel: 1.0,
            surface_kind: SurfaceKind::default(),
            surface_style: SurfaceStyle::default(),
            surface_pattern_spacing: 1.5,
            surface_pattern_width_pixels: 1.25,
            ribbon_width: 1.2,
            tube_radius: 0.3,
            tube_radius_mapping: TubeRadiusMapping::Constant,
            point_size_pixels: 3.0,
            line_width_pixels: 1.5,
        }
    }
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
}
impl Representation {
    /// A representation of the given kind over a selection, with defaults
    /// tuned per kind.
    #[must_use]
    pub fn new(target: RepresentationTarget, kind: RepresentationKind) -> Self {
        let mut params = RepresentationParams::default();
        match kind {
            RepresentationKind::BallAndStick => {
                // Classic ball-and-stick proportions: small spheres, thin bonds.
                params.radius_scale = 0.25;
            }
            RepresentationKind::Licorice => {
                params.radius_scale = 1.0;
            }
            RepresentationKind::Surface => {
                params.isolevel = 0.0;
            }
            RepresentationKind::Trace => {
                params.tube_radius = 0.12;
            }
            RepresentationKind::PaperChain => {
                params.ribbon_width = 0.0;
            }
            _ => {}
        }
        Self {
            target,
            kind,
            color: ColorScheme::default(),
            appearance: None,
            material: Material::default(),
            params,
            volume: VolumeStyle::default(),
            segmentation: SegmentationStyle::default(),
            surface_scalar: None,
            clipping: ClipSet::default(),
            visible: true,
            order: 0,
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
    }
}
