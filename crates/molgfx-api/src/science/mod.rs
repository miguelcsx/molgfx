//! Declarative scientific scene items whose bulk data stays in runtime bindings.

use crate::id::{
    AnnotationId, MeasurementId, ScientificInteractionId, StructureId, TrajectoryId, VolumeId,
};
use crate::representation::Selection;
use crate::representation::private::Sealed;
use crate::{Color, Error, Scene, SceneItem};
use serde::{Deserialize, Serialize};

#[cfg(test)]
mod tests;

pub(crate) mod bindings;
mod builders;
pub(crate) mod lower;
mod validation;
pub(crate) use bindings::ScienceBindings;
pub use bindings::{ScientificHandles, VolumeBinding};
pub use builders::{annotation, density, interaction, measurement, trajectory};

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

/// Scientific interaction classification.
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

/// Caller-supplied scientific interaction specification.
///
/// Detection is molecular analysis and belongs to `molframe`; this crate stores
/// and presents interactions the caller has already resolved.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum ScientificInteractionSpec {
    /// Caller-provided interaction endpoints.
    Explicit {
        /// Scientific interaction class.
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

impl Sealed for ScientificInteractionSpec {}

impl SceneItem for ScientificInteractionSpec {
    type Id = ScientificInteractionId;

    fn add_to(self, scene: &mut Scene) -> Result<Self::Id, Error> {
        scene.insert_scientific_interaction(self)
    }
}

impl Sealed for TrajectorySpec {}

impl SceneItem for TrajectorySpec {
    type Id = TrajectoryId;

    fn add_to(self, scene: &mut Scene) -> Result<Self::Id, Error> {
        scene.insert_trajectory(self)
    }
}
