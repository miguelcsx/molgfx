//! Declarative overlay scene items whose bulk data stays in runtime bindings.

use crate::id::{
    AnnotationId, EllipsoidId, InteractionId, MeasurementId, PlaneId, StructureId, TrajectoryId,
    VolumeId,
};
use crate::representation::Selection;
use crate::representation::private::Sealed;
use crate::{Color, Error, Scene, SceneItem};
use serde::{Deserialize, Serialize};

#[cfg(test)]
mod surfaces_tests;
#[cfg(test)]
mod tests;

pub(crate) mod bindings;
mod builders;
pub(crate) mod lower;
mod lower_guides;
#[cfg(test)]
mod lower_tests;
mod planes;
mod surfaces;
mod validation;
pub(crate) use bindings::OverlayBindings;
pub use bindings::{
    OverlayHandles, TrajectoryBinding, TrajectoryFrame, VolumeBinding, VolumeStatistics,
};
pub use builders::{annotation, density, ellipsoid, interaction, measurement, trajectory};
pub use planes::PlaneSpec;
pub use surfaces::{
    AssemblyInstance, AssemblySpec, FitResult, MovieExportRequest, UnitCellSpec, ValidationFinding,
};

/// Portable origin for bulk data stored outside [`crate::SceneSpec`].
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct DataSource {
    /// Stable content identity; empty only while a runtime-only binding is unresolved.
    pub content_hash: Box<str>,
    /// Optional portable location.
    pub uri: Option<Box<str>>,
    /// Optional format hint.
    pub format: Option<Box<str>>,
}

impl DataSource {
    /// Describes content-addressed data with an optional portable location.
    #[must_use]
    pub fn new(content_hash: impl Into<Box<str>>) -> Self {
        Self {
            content_hash: content_hash.into(),
            uri: None,
            format: None,
        }
    }

    /// Sets a portable URI without loading data.
    #[must_use]
    pub fn uri(mut self, uri: impl Into<Box<str>>) -> Self {
        self.uri = Some(uri.into());
        self
    }

    /// Sets a format hint.
    #[must_use]
    pub fn format(mut self, format: impl Into<Box<str>>) -> Self {
        self.format = Some(format.into());
        self
    }
}

/// A semantic point used by labels, measurements, and explicit interactions.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Anchor {
    /// Fixed world-space point.
    World {
        /// Position in the scene's world coordinate system.
        position: [f32; 3],
    },
    /// Centroid of a molecular query on one structure.
    Selection {
        /// Molecular source containing the query target.
        structure: StructureId,
        /// Query whose centroid defines the anchor.
        selection: Selection,
    },
}

/// Density-volume metadata. Grid values are supplied through a runtime binding.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct VolumeSpec {
    /// Portable source descriptor for the grid values.
    pub source: DataSource,
    /// Voxel dimensions along x, y, and z.
    pub dimensions: [u32; 3],
    /// Voxel spacing in ångström.
    pub spacing: [f32; 3],
    /// World-space origin in ångström.
    pub origin: [f32; 3],
    /// Density isovalue used by the default presentation.
    pub isovalue: f32,
    /// Default presentation color.
    pub color: Color,
}

/// Immutable density-volume builder.
#[derive(Clone, PartialEq, Debug)]
pub struct Volume(VolumeSpec);

impl Volume {
    /// Sets positive voxel spacing in ångström.
    #[must_use]
    pub fn spacing(mut self, spacing: [f32; 3]) -> Self {
        self.0.spacing = spacing;
        self
    }

    /// Sets the world-space grid origin.
    #[must_use]
    pub fn origin(mut self, origin: [f32; 3]) -> Self {
        self.0.origin = origin;
        self
    }

    /// Sets the default finite isovalue.
    #[must_use]
    pub fn isovalue(mut self, isovalue: f32) -> Self {
        self.0.isovalue = isovalue;
        self
    }

    /// Sets the default volume color.
    #[must_use]
    pub fn color(mut self, color: Color) -> Self {
        self.0.color = color;
        self
    }
}

/// Label or annotation specification.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct AnnotationSpec {
    /// Semantic placement of the label.
    pub anchor: Anchor,
    /// Display text.
    pub text: Box<str>,
    /// Label color.
    pub color: Color,
}

/// Immutable label builder.
#[derive(Clone, PartialEq, Debug)]
pub struct Label(AnnotationSpec);

impl Label {
    /// Sets the label color.
    #[must_use]
    pub fn color(mut self, color: Color) -> Self {
        self.0.color = color;
        self
    }
}

/// Geometric measurement with exact arity encoded by its tagged form.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MeasurementSpec {
    /// Distance between two anchors.
    Distance {
        /// Ordered endpoints.
        anchors: [Anchor; 2],
    },
    /// Angle formed by three anchors.
    Angle {
        /// Ordered angle points.
        anchors: [Anchor; 3],
    },
    /// Signed torsion formed by four anchors.
    Dihedral {
        /// Ordered torsion points.
        anchors: [Anchor; 4],
    },
}

/// The anchors a measurement reads, in order.
#[must_use]
pub fn measurement_anchors(spec: &MeasurementSpec) -> &[Anchor] {
    match spec {
        MeasurementSpec::Distance { anchors } => anchors,
        MeasurementSpec::Angle { anchors } => anchors,
        MeasurementSpec::Dihedral { anchors } => anchors,
    }
}

/// The kind name and anchor count of a measurement.
#[must_use]
pub const fn measurement_shape(spec: &MeasurementSpec) -> (&'static str, usize) {
    match spec {
        MeasurementSpec::Distance { .. } => ("distance", 2),
        MeasurementSpec::Angle { .. } => ("angle", 3),
        MeasurementSpec::Dihedral { .. } => ("dihedral", 4),
    }
}

/// Overlay interaction classification.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InteractionKind {
    /// Directional donor-acceptor hydrogen bond.
    HydrogenBond,
    /// Oppositely charged residue contact.
    SaltBridge,
    /// Aromatic ring stacking.
    PiStacking,
    /// Cation-aromatic contact. `molframe` has no cation-π class, so this
    /// authoring kind lowers onto [`Self::PiStacking`], the aromatic-ring
    /// interaction it specializes.
    CationPi,
    /// Hydrophobic contact.
    Hydrophobic,
    /// Metal-ligand coordination.
    MetalCoordination,
    /// Unclassified spatial contact. `molframe` has no unclassified class, so
    /// this authoring kind lowers onto [`Self::Hydrophobic`], its non-polar
    /// contact.
    Contact,
}

impl InteractionKind {
    /// Nearest `molframe` interaction class this authoring kind lowers onto.
    ///
    /// Two authoring kinds have no exact upstream counterpart: `CationPi`
    /// lowers onto [`molgfx_core::InteractionKind::PiStacking`] and `Contact`
    /// onto [`molgfx_core::InteractionKind::Hydrophobic`]. A covalent
    /// disulfide bridge is not a contact interaction at all, so it is not an
    /// authorable kind rather than being mapped onto a class it contradicts.
    #[must_use]
    pub const fn core_kind(self) -> molgfx_core::InteractionKind {
        match self {
            Self::HydrogenBond => molgfx_core::InteractionKind::HydrogenBond,
            Self::SaltBridge => molgfx_core::InteractionKind::SaltBridge,
            Self::PiStacking | Self::CationPi => molgfx_core::InteractionKind::PiStacking,
            Self::Hydrophobic | Self::Contact => molgfx_core::InteractionKind::Hydrophobic,
            Self::MetalCoordination => molgfx_core::InteractionKind::MetalCoordination,
        }
    }
}

/// Caller-supplied overlay interaction specification.
///
/// Detection is molecular analysis and belongs to `molframe`; this crate stores
/// and presents interactions the caller has already resolved.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum InteractionSpec {
    /// Caller-provided interaction endpoints.
    Explicit {
        /// Overlay interaction class.
        kind: InteractionKind,
        /// Ordered endpoints.
        endpoints: [Anchor; 2],
    },
}

/// Trajectory metadata. Frames are supplied by a runtime binding or data source.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct TrajectorySpec {
    /// Structure whose topology owns each frame.
    pub structure: StructureId,
    /// Portable source descriptor for frame data.
    pub source: DataSource,
    /// Declared number of frames.
    pub frame_count: u64,
    /// Optional positive interval between frames.
    pub time_step: Option<f64>,
    /// Optional physical unit for `time_step`.
    pub time_unit: Option<Box<str>>,
}

/// Per-atom anisotropic-displacement ellipsoid overlay.
///
/// Every selected atom that carries a displacement tensor in its source is
/// drawn as one ellipsoid, so the item is a selection plus a display style
/// rather than a per-atom list. Atoms without a tensor are simply skipped.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct EllipsoidSpec {
    /// Structure whose per-atom tensors the selection reads.
    pub structure: StructureId,
    /// Atoms to draw; only those carrying a tensor produce geometry.
    pub selection: Selection,
    /// Multiplier on the displacement tensor.
    ///
    /// The tensor is a mean-square displacement, so the drawn surface is one
    /// standard deviation; a larger scale widens the ellipsoid by that factor.
    /// Applied as `scale² · U`, since lengths scale with the square root of a
    /// displacement tensor's eigenvalues.
    #[serde(default = "default_ellipsoid_scale")]
    pub scale: f32,
    /// Display color.
    pub color: Color,
    /// Final opacity in `[0, 1]`.
    #[serde(default = "default_opacity")]
    pub opacity: f32,
}

/// The one-standard-deviation surface an unscaled tensor already describes.
fn default_ellipsoid_scale() -> f32 {
    1.0
}

fn default_opacity() -> f32 {
    1.0
}

macro_rules! tuple_scene_item {
    ($item:ty, $id:ty, $method:ident) => {
        impl Sealed for $item {}

        impl SceneItem for $item {
            type Id = $id;

            fn add_to(self, scene: &mut Scene) -> Result<Self::Id, Error> {
                scene.$method(self.0)
            }
        }
    };
}

tuple_scene_item!(Volume, VolumeId, insert_volume);
tuple_scene_item!(Label, AnnotationId, insert_annotation);

impl Sealed for MeasurementSpec {}

impl SceneItem for MeasurementSpec {
    type Id = MeasurementId;

    fn add_to(self, scene: &mut Scene) -> Result<Self::Id, Error> {
        scene.insert_measurement(self)
    }
}

impl Sealed for InteractionSpec {}

impl SceneItem for InteractionSpec {
    type Id = InteractionId;

    fn add_to(self, scene: &mut Scene) -> Result<Self::Id, Error> {
        scene.insert_interaction(self)
    }
}

impl Sealed for EllipsoidSpec {}

impl SceneItem for EllipsoidSpec {
    type Id = EllipsoidId;

    fn add_to(self, scene: &mut Scene) -> Result<Self::Id, Error> {
        scene.insert_ellipsoids(self)
    }
}

impl Sealed for PlaneSpec {}

impl SceneItem for PlaneSpec {
    type Id = PlaneId;

    fn add_to(self, scene: &mut Scene) -> Result<Self::Id, Error> {
        scene.insert_plane(self)
    }
}

impl Sealed for TrajectorySpec {}

impl SceneItem for TrajectorySpec {
    type Id = TrajectoryId;

    fn add_to(self, scene: &mut Scene) -> Result<Self::Id, Error> {
        scene.insert_trajectory(self)
    }
}
