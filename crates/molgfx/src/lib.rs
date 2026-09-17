//! Host-neutral facade for the public engine surface.
//!
//! Callers import from here only. Scene construction, representations and
//! rendering are separate concerns with separate types; backends are chosen
//! by capability behind the engine alias and never named. This crate holds
//! no rendering or scientific logic. Math identities are project-owned and
//! reexported here; glam remains a private implementation detail.

#![forbid(unsafe_code)]

pub use molgfx_core::{
    AnalyticCapsule, AnalyticSphere, AnalyticTemplate, AnisotropicEllipsoid, Annotation,
    AnnotationAnchor, AnnotationHandle, AnnotationKind, ArcballController, AtomProperty,
    AtomPropertyHandle, AtomPropertyMeaning, AtomSelection, AttributeChunkPayload, AttributeColumn,
    AttributeDescription, AttributeDescriptor, AttributeHandle, AttributeKind, AttributeValues,
    BondTopologyFrame, BondTopologySegment, BoolExpr, BrickAddress, BrickCatalog, BrickDescriptor,
    BrickId, BrickMetadata, BrickShape, BrickValueRange, Button, CameraBookmark, CameraEasing,
    CameraKeyframe, CameraPath, CarbohydrateShape, CarbohydrateSymbol, CatalogChildren,
    CatalogRoots, ChunkBounds, ChunkData, ChunkDescriptor, ChunkDomainKind, ChunkDomainRef,
    ChunkEntityRef, ChunkFootprint, ChunkId, ChunkPayload, ChunkSpan, ChunkSpatialKind, ClipCap,
    ClipPlane, ClipSet, ColorExpr, ColorParameter, ColorScheme, ContentAddress, CoreError,
    CrystalCell, DatasetCatalog, DatasetError, DatasetId, DeviceLossReport, DirtyGeneration,
    DomainVisualDescription, EntityKind, EntityProvenance, EntityProvenance as Provenance,
    EntityRef, EntitySelection, EntitySelectionRows, Eviction, FaceVisibility, FailureReason,
    FlyController, GenericSceneDescriptionSources, GlobalPickIdentity, GpuEntitySelection, Guide,
    GuideCap, GuideHandle, GuideStyle, HostWorkingSet, HostWorkingSetError, InputEvent,
    InstanceBatch, InstanceBatchDescription, InstanceBatchHandle, InstanceChunkPayload,
    InstanceStyle, Key, LocalRow, LogicalRow, MAX_CLIP_PLANES, MAX_VISUAL_INSTRUCTIONS,
    MAX_VISUAL_PARAMETERS, MAX_VISUAL_PROPERTIES, MAX_VOLUME_TRANSFER_POINTS, ManifestError,
    MarkerShape, MarkerStyle, Material, MaterialModel, Measurement, MeasurementHandle,
    MeasurementKind, Mesh, MeshDescription, MeshHandle, MeshInstance, MeshInstanceDescription,
    MeshInstanceHandle, MeshTopology, MeshVertex, OccupancyStream, OrbitController, OverlayAnchor,
    OverlayContent, OverlayDescription, OverlayHandle, PagedRelation, PagedSpatialAnchor, Particle,
    ParticleBoundary, ParticleMotion, ParticleMotionDescription, ParticleShape, PayloadKind,
    PayloadReference, PlanarRegion, PlaybackMode, PointBatch, PointBatchDescription,
    PointBatchHandle, PointChunkPayload, PointFramePair, PointGlyph, PointStyle, PolylineKind,
    Primitive, PrimitiveDescription, PrimitiveHandle, PropertyComparison, PropertyLegend,
    ProvenanceDetail, Quadric, ReferencedPayloadKind, Relation, RelationBatch,
    RelationBatchDescription, RelationBatchHandle, RelationChunkPayload, RelationDependency,
    RelationLayout, RelationPartition, RelationPattern, RelationStyle, Representation,
    RepresentationConfig, RepresentationHandle, RepresentationInput, RepresentationKind,
    RepresentationParams, RepresentationPreset, RepresentationTarget, ResidencyBudget,
    ResidencyClass, ResidencyDetail, ResidencyError, ResidencyKey, ResidencyMachine,
    ResidencyOutput, ResidencyPhase, ResidencyRequest, ResidencySnapshot, ResidencyTicket,
    RigidInstance, RowDomain, RowDomainDescription, RowEntityRef, ScalarContours, ScalarExpr,
    ScalarFieldSemantics, ScalarParameter, ScalarRamp, ScalarVolume, Scene, SceneDescription,
    SceneDescriptionSources, SceneManifest, ScreenOverlay, SecondaryStructure, SegmentStyle,
    SegmentStyleTable, SegmentationHandle, SegmentationStyle, SegmentedVolume, Select,
    SelectionHandle, SourceNamespace, SourceRows, SourceRowsDescription, SpatialAnchor,
    StaleCompletion, StructureHandle, SurfaceComponentPolicy, SurfaceKind, SurfaceScalarOverlay,
    SurfaceStyle, SymmetryInstance, TemplatePartChunkRef, TemplatePartPick, TemplatePartRef,
    TimeWarp, Timeline, TimelineTrackHandle, TopologyBond, TrajectoryBranch, TrajectoryFrame,
    TrajectorySegment, TrajectoryStateGraph, TubeRadiusMapping, Usage, ValidationKind,
    ValidationMarker, VectorExpr, VectorParameter, VisualAttributeDescription, VisualAttributeRef,
    VisualCompatibility, VisualDescriptor, VisualError, VisualEvaluation, VisualInputs,
    VisualInstructionGpu, VisualOutput, VisualProgram, VisualProgramBuilder, VisualStage,
    VisualStyle, VolumeHandle, VolumeRegion, VolumeRendering, VolumeSegmentRef, VolumeSlice,
    VolumeStyle, VolumeTransferFunction, VolumeTransferPoint, read_manifest, write_manifest,
};
pub use molgfx_gpu::{
    Capabilities, FenceValue, GpuError, PowerPreference, UploadRingConfig, WindowSource,
    WindowTarget,
};
pub use molgfx_math::{Aabb, BoundingSphere, Camera, Mat4, Projection, Quat, Rgba8, Vec3};
#[cfg(feature = "realtime")]
pub use molgfx_render::{
    AttributeChunkWindow, BackdropStyle, BloomStyle, BondChunkPlacement, BrickAtlasConfig,
    BrickAtlasError, BrickAtlasKind, BrickAtlasMetrics, BrickAtlasUpload, ChunkPlacementError,
    ChunkPlacementId, ChunkPlacementStatus, ChunkRepresentation, ChunkResidencyError,
    ChunkResidencyMetrics, DepthOfField, DerivedCacheBudget, DerivedCacheUsage, DisplayGamut,
    DisplayTransform, EffectLayer, EngineConfig, FocusTarget, FrameCompleteness, FrameDegradation,
    FrameMetrics, FrameReport, FrameStatus, FrameTiming, HdrImage, IllustrationStyle, Image,
    ImageConfig, InstanceChunkPlacement, InstanceChunkWindow, LightingEnvironment, MotionBlur,
    Pick, PickEntity, PointChunkPlacement, PresentationEffect, RelationChunkPlacement, RenderError,
    RenderMode, RenderProfile, RenderSession, ResidencyConfig, ResidentGenericChunk,
    ResidentStructureChunk, ResidentTrajectoryChunk, ResolvedRenderPlan, StructureChunkPlacement,
    ToneMapping, TrajectoryChunkWindow, TransferFunction,
};
#[cfg(all(feature = "realtime", not(target_arch = "wasm32")))]
pub use molgfx_render::{FrameTicket, SequenceConfig, SequenceFrame};

/// The engine over the default backend, selected by capability.
#[cfg(feature = "realtime")]
pub type Engine = molgfx_render::Engine<molgfx_wgpu::WgpuDevice>;

/// Bounded off-screen sequence pipeline over the capability-selected backend.
#[cfg(all(feature = "realtime", not(target_arch = "wasm32")))]
pub type SequenceRenderer = molgfx_render::SequenceRenderer<molgfx_wgpu::WgpuDevice>;

#[cfg(feature = "semantic")]
pub use molgfx_semantic::{
    ChunkKey, ChunkRequest, CompositionError, DifferenceCompositionStyle, DifferenceLayer,
    EnsembleCompositionStyle, EnsembleLayer, FocusCompositionStyle, FocusLayer,
    GenericCompositionScene, GenericCompositionView, LodCluster, LodClusterKey, LodFrame, LodIndex,
    LodLevel, LodPolicy, LodScene, MappingError, PropertyMapping, StreamPlan, StreamPlanner,
    StreamingBudget, SurfaceZone, SurfaceZoneScene, SurfaceZoneStyle,
};
