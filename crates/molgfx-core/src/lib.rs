//! The semantic scene graph: columnar tables, GPU record layouts, the
//! borrowed coordinate seam.
//!
//! This crate holds data. It knows how bytes must be laid out for the GPU but
//! never issues a device command, so everything here is testable without one.
//! Coordinates are borrowed from the parsed structure and never copied; all
//! other per-atom state lives in owned, revision-counted columns.

#![forbid(unsafe_code)]

#[cfg(test)]
mod fixture;

mod controls;
mod dataset;
mod gpu;
mod representation;
mod scene;
mod selection;
mod serialization;
mod storage;
mod structure;

pub(crate) use controls::input;
pub(crate) use gpu::gpu_types;
pub(crate) use representation::{annotation, radii};
pub(crate) use selection::select;
pub(crate) use storage::{column, error, handle};
pub(crate) use structure::{atoms, coord, density, hierarchy, placed};

pub use controls::{
    ArcballController, Button, CameraBookmark, CameraEasing, CameraKeyframe, CameraPath,
    FlyController, InputEvent, Key, OrbitController,
};
pub use dataset::{
    AttributeChunkPayload, BrickAddress, BrickCatalog, BrickDescriptor, BrickId, BrickMetadata,
    BrickShape, BrickValueRange, CatalogChildren, CatalogRoots, ChunkBounds, ChunkData,
    ChunkDescriptor, ChunkDomainKind, ChunkDomainRef, ChunkEntityRef, ChunkFootprint, ChunkId,
    ChunkOccurrenceId, ChunkPayload, ChunkSpan, ChunkSpatialKind, ChunkVisualBinding,
    ChunkVisualDescriptor, DatasetCatalog, DatasetError, DatasetId, DeviceLossReport,
    DirtyGeneration, Eviction, FailureReason, GlobalPickIdentity, GpuPickToken, HostWorkingSet,
    HostWorkingSetError, InstanceChunkPayload, LabelBrickPayload, LocalRow, LogicalRow,
    MeshChunkPayload, OccupancyBrickPayload, PagedPickResolver, PagedRelation, PagedSpatialAnchor,
    PayloadKind, PickGeneration, PickPageDescriptor, PickPageTicket, PickReadback, PickingError,
    PointChunkPayload, PropertyChunkPayload, PropertyValues, ProviderBridgeError,
    ProviderDatasetBridge, ProxyChunkPayload, RelationChunkPayload, ResidencyBudget,
    ResidencyClass, ResidencyDetail, ResidencyError, ResidencyKey, ResidencyMachine,
    ResidencyOutput, ResidencyPhase, ResidencyRequest, ResidencySnapshot, ResidencyTicket,
    ResidentPage, ScalarChunkPayload, StaleCompletion, StructureAsset, StructureAssetPlacement,
    StructureChunkPayload, TemplatePartChunkRef, TrajectoryChunkPayload, TrajectoryFramesPayload,
    Usage, VolumeBrickPayload,
};
pub use gpu::{
    AtomFlags, AtomGpu, BondGpu, DrawIndirectArgs, EntityId, EntityIdError, EntityKind, EntityRef,
    InteractionGpu, ParticleMotionGpu, PrimitiveGpu, VolumeSegmentRef,
};
pub use representation::{
    AnchorLayout, Annotation, AnnotationAnchor, AnnotationKind, AtomProperty, AtomPropertyMeaning,
    AttributeColumn, AttributeDescriptor, AttributeKind, AttributeValues, BoolExpr, ColorExpr,
    ColorParameter, ColorScheme, FaceVisibility, Guide, GuideCap, GuideStyle, InteractionAnchor,
    InteractionDirection, InteractionEdge, InteractionGeometry, InteractionKind,
    InteractionPattern, InteractionStyle, MAX_MESH_VERTICES, MAX_VISUAL_INSTRUCTIONS,
    MAX_VISUAL_PARAMETERS, MAX_VISUAL_PROPERTIES, MAX_VOLUME_TRANSFER_POINTS, MarkerShape,
    MarkerStyle, Material, MaterialModel, Measurement, MeasurementKind, Mesh, MeshInstance,
    MeshTopology, MeshVertex, OverlayAnchor, OverlayContent, PolylineKind, PropertyAppearance,
    PropertyAppearanceSample, PropertyLegend, Quadric, Relation, RelationBatch, RelationDependency,
    RelationLayout, RelationPartition, RelationPattern, RelationStyle, Representation,
    RepresentationConfig, RepresentationKind, RepresentationParams, RepresentationPreset,
    RepresentationTarget, ScalarContours, ScalarExpr, ScalarFieldSemantics, ScalarParameter,
    ScalarRamp, ScreenOverlay, SpatialAnchor, SurfaceComponentPolicy, SurfaceComponentPolicyError,
    SurfaceComponentThreshold, SurfaceKind, SurfaceScalarOverlay, SurfaceStyle, TemplatePartRef,
    TubeRadiusMapping, VectorExpr, VectorParameter, VisualAttributeRef, VisualColumnKey,
    VisualCompatibility, VisualDescriptor, VisualError, VisualEvaluation, VisualInputs,
    VisualInstructionGpu, VisualOutput, VisualProgram, VisualProgramBuilder, VisualStage,
    VisualStyle, VolumeRegion, VolumeRendering, VolumeSlice, VolumeStyle, VolumeTransferFunction,
    VolumeTransferPoint, cpk_color, vdw_radius,
};
pub use scene::{
    AtomCorrespondence, DifferenceScene, DifferenceStyle, DifferenceView, InstanceFramePair,
    PlaybackMode, PointFramePair, RepresentationInput, RowDomain, RowEntityRef, Scene,
    SourceNamespace, SourceRows, TemplatePartPick, TimeWarp, Timeline, TrajectoryBranch,
    TrajectoryStateGraph,
};
pub use selection::{
    AtomSelection, ClipCap, ClipPlane, ClipSet, EntitySelection, EntitySelectionRows,
    GpuEntitySelection, MAX_CLIP_PLANES, PropertyComparison, Select,
};
pub use serialization::{
    AnchorDescription, AnnotationDescription, AtomPropertyDescription, AttributeDescription,
    ClipDescription, ColorDescription, ContentAddress, DomainVisualDescription, EntityDescription,
    GenericSceneDescriptionSources, GuideDescription, GuideStyleDescription,
    InstanceBatchDescription, InteractionDescription, LazyManifest, LigandPoseBatchDescription,
    LigandPoseDescription, ManifestError, MarkerStyleDescription, MaterialDescription,
    MeasurementDescription, MeshDescription, MeshInstanceDescription, ObjectIdentity,
    OccupancyDescription, OverlayDescription, ParticleMotionDescription, PayloadReference,
    PayloadResolver, PointBatchDescription, PrimitiveDescription, PropertyAppearanceDescription,
    ReferencedPayloadKind, RegionDescription, RelationBatchDescription, RepresentationDescription,
    RowDomainDescription, ScalarSemanticsDescription, SceneDescription, SceneDescriptionSources,
    SceneManifest, SegmentStyleDescription, SegmentationStyleDescription, SelectionDescription,
    SelectionMask, SourceRowsDescription, StructureDescription, SurfaceComponentDescription,
    SurfaceScalarDescription, TableCounts, TargetDescription, VerifiedPayload,
    VisualAttributeDescription, VisualInstructionDescription, VisualStyleDescription,
    VolumeDescription, VolumeStyleDescription, VolumeTransferPointDescription, read_manifest,
    write_manifest,
};
pub use storage::{
    AnnotationHandle, AtomPropertyHandle, AttributeHandle, Column, CoreError, EnsembleHandle,
    GuideHandle, InstanceBatchHandle, InteractionHandle, LigandPoseBatchHandle, MeasurementHandle,
    MeshHandle, MeshInstanceHandle, OverlayHandle, PointBatchHandle, PrimitiveHandle,
    RelationBatchHandle, RepresentationHandle, Revision, SegmentationHandle, SelectionHandle,
    StructureHandle, TimelineTrackHandle, VolumeHandle,
};
pub use structure::{
    ActiveTopologyBond, AnalyticCapsule, AnalyticSphere, AnalyticTemplate, AnisotropicEllipsoid,
    AtomTable, BondTopologyFrame, BondTopologySegment, CarbohydrateShape, CarbohydrateSymbol,
    CoordRef, CrystalCell, Ensemble, EntityProvenance, Hierarchy, InstanceBatch, InstanceStyle,
    LicoriceTemplate, LigandPose, LigandPoseBatch, OccupancyStream, Particle, ParticleBoundary,
    ParticleMotion, ParticleShape, PlacedStructure, PlanarRegion, PointBatch, PointGlyph,
    PointStyle, Primitive, ProvenanceDetail, RigidInstance, ScalarVolume, SecondaryStructure,
    SegmentStyle, SegmentStyleTable, SegmentationStyle, SegmentedVolume, SymmetryInstance,
    TopologyBond, TrajectoryFrame, TrajectorySegment, ValidationKind, ValidationMarker,
};
