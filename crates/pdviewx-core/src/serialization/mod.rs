//! Stable scene descriptions and manifest conversion.

mod records;
mod rehydrate;
mod types;

#[path = "serialization.rs"]
pub(crate) mod manifest;

pub use types::{
    AnchorDescription, AnnotationDescription, AtomPropertyDescription, ClipDescription,
    ColorDescription, EntityDescription, GuideDescription, GuideStyleDescription,
    InteractionDescription, MarkerStyleDescription, MaterialDescription, MeasurementDescription,
    MeshDescription, MeshInstanceDescription, ObjectIdentity, OverlayDescription,
    ParticleMotionDescription, PrimitiveDescription, PropertyAppearanceDescription,
    RegionDescription, RepresentationDescription, ScalarSemanticsDescription, SceneDescription,
    SceneManifest, SegmentStyleDescription, SegmentationStyleDescription, SelectionDescription,
    SelectionMask, StructureDescription, SurfaceScalarDescription, TableCounts, TargetDescription,
    VolumeDescription, VolumeStyleDescription, VolumeTransferPointDescription,
};

/// Caller-owned payloads required to rehydrate a scene description.
///
/// Structures and dense grids remain outside the JSON manifest. Slices are
/// matched in manifest order and every payload is fingerprinted before any
/// scene table is mutated.
#[derive(Clone, Copy, Debug)]
pub struct SceneDescriptionSources<'a> {
    /// Source structures in manifest order.
    pub structures: &'a [pdbiox::Structure],
    /// Scalar volumes in manifest order.
    pub volumes: &'a [crate::DensityVolume],
    /// Categorical volumes in manifest order.
    pub segmentations: &'a [crate::SegmentedVolume],
    /// Atom properties in manifest order.
    pub atom_properties: &'a [crate::AtomProperty],
    /// Caller mesh sources in manifest order.
    pub meshes: &'a [crate::Mesh],
}
