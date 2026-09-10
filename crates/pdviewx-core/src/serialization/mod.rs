//! Stable scene descriptions and manifest conversion.

#[cfg(test)]
mod asset_serialization_tests;
mod coordinate_hash;
mod generic_description;
mod generic_records;
#[cfg(test)]
mod generic_serialization_tests;
mod ligand_pose_description;
mod manifest_io;
mod records;
mod rehydrate;
mod types;

#[path = "serialization.rs"]
pub(crate) mod manifest;

pub use generic_description::{
    AttributeDescription, DomainVisualDescription, InstanceBatchDescription, PointBatchDescription,
    RelationBatchDescription, RowDomainDescription, SourceRowsDescription,
};
pub use ligand_pose_description::{LigandPoseBatchDescription, LigandPoseDescription};
pub use manifest_io::{
    ContentAddress, LazyManifest, ManifestError, PayloadReference, PayloadResolver,
    ReferencedPayloadKind, SceneManifest, VerifiedPayload, read_manifest, write_manifest,
};
pub use types::{
    AnchorDescription, AnnotationDescription, AtomPropertyDescription, ClipDescription,
    ColorDescription, EntityDescription, GuideDescription, GuideStyleDescription,
    InteractionDescription, MarkerStyleDescription, MaterialDescription, MeasurementDescription,
    MeshDescription, MeshInstanceDescription, ObjectIdentity, OccupancyDescription,
    OverlayDescription, ParticleMotionDescription, PrimitiveDescription,
    PropertyAppearanceDescription, RegionDescription, RepresentationDescription,
    ScalarSemanticsDescription, SceneDescription, SegmentStyleDescription,
    SegmentationStyleDescription, SelectionDescription, SelectionMask, StructureDescription,
    SurfaceComponentDescription, SurfaceScalarDescription, TableCounts, TargetDescription,
    VisualAttributeDescription, VisualInstructionDescription, VisualStyleDescription,
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
    pub volumes: &'a [crate::ScalarVolume],
    /// Categorical volumes in manifest order.
    pub segmentations: &'a [crate::SegmentedVolume],
    /// Atom properties in manifest order.
    pub atom_properties: &'a [crate::AtomProperty],
    /// Caller mesh sources in manifest order.
    pub meshes: &'a [crate::Mesh],
}

/// Generic immutable payloads required by schema-8 row tables.
///
/// Each slice is matched in manifest order. Values may share their original
/// `Arc` storage; reconstruction validates their content address before
/// inserting them at the exact generational identity recorded by the scene.
#[derive(Clone, Copy, Debug, Default)]
pub struct GenericSceneDescriptionSources<'a> {
    /// Generic point batches in manifest order.
    pub point_batches: &'a [crate::PointBatch],
    /// Shared-template instance batches in manifest order.
    pub instance_batches: &'a [crate::InstanceBatch],
    /// Typed immutable columns in manifest order.
    pub attributes: &'a [crate::AttributeColumn],
    /// Generic relation batches in manifest order.
    pub relation_batches: &'a [crate::RelationBatch],
}
