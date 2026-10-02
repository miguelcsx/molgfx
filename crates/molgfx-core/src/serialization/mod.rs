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

mod sources;

pub use sources::{GenericSceneDescriptionSources, SceneDescriptionSources};
