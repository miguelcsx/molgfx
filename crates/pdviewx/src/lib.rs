//! Facade crate: feature-gated re-exports of the public engine surface.
//!
//! Callers import from here only. Scene construction, representations and
//! rendering are separate concerns with separate types; backends are chosen
//! by capability behind the engine alias and never named. This crate holds
//! no logic.

#![forbid(unsafe_code)]

pub use pdviewx_core::{
    AnisotropicEllipsoid, Annotation, AnnotationAnchor, AnnotationHandle, AnnotationKind,
    ArcballController, AtomProperty, AtomPropertyHandle, AtomPropertyMeaning, AtomSelection,
    Button, CarbohydrateShape, CarbohydrateSymbol, ClipCap, ClipPlane, ClipSet, ColorScheme,
    CoreError, CrystalCell, DensityVolume, Ensemble, EnsembleHandle, EntityKind, EntityProvenance,
    EntityProvenance as Provenance, EntityRef, FaceVisibility, FlyController, Guide, GuideCap,
    GuideHandle, GuideStyle, InputEvent, InteractionAnchor, InteractionDirection, InteractionEdge,
    InteractionGeometry, InteractionHandle, InteractionKind, InteractionPattern, InteractionStyle,
    Key, MAX_CLIP_PLANES, MAX_VOLUME_TRANSFER_POINTS, MarkerShape, MarkerStyle, Material,
    MaterialModel, Measurement, MeasurementHandle, MeasurementKind, Mesh, MeshDescription,
    MeshHandle, MeshInstance, MeshInstanceDescription, MeshInstanceHandle, MeshTopology,
    MeshVertex, OrbitController, OverlayAnchor, OverlayContent, OverlayDescription, OverlayHandle,
    Particle, ParticleBoundary, ParticleMotion, ParticleMotionDescription, ParticleShape,
    PlanarRegion, PolylineKind, Primitive, PrimitiveDescription, PrimitiveHandle,
    PropertyAppearance, PropertyAppearanceSample, PropertyComparison, PropertyLegend,
    ProvenanceDetail, Quadric, Representation, RepresentationConfig, RepresentationHandle,
    RepresentationInput, RepresentationKind, RepresentationParams, RepresentationPreset,
    RepresentationTarget, ScalarContours, ScalarFieldSemantics, ScalarRamp, Scene,
    SceneDescription, SceneDescriptionSources, SceneManifest, ScreenOverlay, SecondaryStructure,
    SegmentStyle, SegmentStyleTable, SegmentationHandle, SegmentationStyle, SegmentedVolume,
    Select, SelectionHandle, StructureHandle, SurfaceComponentPolicy, SurfaceKind,
    SurfaceScalarOverlay, SurfaceStyle, SymmetryInstance, TrajectoryFrame, TrajectorySegment,
    TubeRadiusMapping, ValidationKind, ValidationMarker, VolumeHandle, VolumeRegion,
    VolumeRendering, VolumeSegmentRef, VolumeSlice, VolumeStyle, VolumeTransferFunction,
    VolumeTransferPoint,
};
pub use pdviewx_gpu::{Capabilities, GpuError, WindowSource, WindowTarget};
pub use pdviewx_math::{Aabb, BoundingSphere, Camera, Mat4, Projection, Quat, Rgba8, Vec3};

#[cfg(feature = "realtime")]
pub use pdviewx_render::{
    BackdropStyle, BloomStyle, DepthOfField, DisplayGamut, DisplayTransform, EffectLayer,
    EngineConfig, FocusTarget, FrameOutcome, FrameTiming, IllustrationStyle, Image, ImageConfig,
    LightingEnvironment, MotionBlur, Pick, PickEntity, PresentationEffect, RenderError, RenderMode,
    RenderProfile, RenderSession, ResolvedRenderPlan, ToneMapping, TransferFunction,
};

/// The engine over the default backend, selected by capability.
#[cfg(feature = "realtime")]
pub type Engine = pdviewx_render::Engine<pdviewx_gpu_wgpu::WgpuDevice>;

#[cfg(feature = "semantic")]
pub use pdviewx_semantic::{
    AtomCorrespondence, ChunkKey, ChunkRequest, DifferenceScene, DifferenceStyle, DifferenceView,
    DistanceBands, EnsembleScene, EnsembleStyle, EnsembleView, FocusBand, FocusContext, FocusError,
    FocusScene, FocusStyle, FocusSurfaceExtent, FocusView, LodCluster, LodClusterKey, LodFrame,
    LodIndex, LodLevel, LodPolicy, MappingError, ProbabilityCloudView, PropertyMapping, StreamPlan,
    StreamPlanner, StreamingBudget, SurfaceZone, SurfaceZoneScene, SurfaceZoneStyle,
};
