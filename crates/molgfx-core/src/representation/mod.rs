//! Declarative visual components and caller-authored render annotations.

pub(crate) mod annotation;
pub(crate) mod atom_property;
mod attribute;
mod color_overlay;
mod config;
pub(crate) mod guide;
pub(crate) mod interaction;
#[path = "representation.rs"]
pub(crate) mod kinds;
pub(crate) mod material;
pub(crate) mod mesh;
pub(crate) mod mesh_instance;
pub(crate) mod overlay;
pub(crate) mod quadric;
pub(crate) mod radii;
mod relation;
pub(crate) mod scalar;
mod surface_components;
pub(crate) mod tube_radius;
pub(crate) mod visual;
mod visual_descriptor;
mod volume_style;

pub use annotation::{
    Annotation, AnnotationAnchor, AnnotationKind, MarkerShape, MarkerStyle, Measurement,
    MeasurementKind,
};
pub use atom_property::{
    AtomProperty, AtomPropertyMeaning, PropertyAppearance, PropertyAppearanceSample, PropertyLegend,
};
pub use attribute::{AttributeColumn, AttributeDescriptor, AttributeKind, AttributeValues};
pub use color_overlay::{ColorOverlay, MAX_COLOR_OVERLAY_CLASSES};
pub use config::RepresentationConfig;
pub use guide::{Guide, GuideCap, GuideStyle, PolylineKind};
pub use interaction::{
    InteractionAnchor, InteractionDirection, InteractionEdge, InteractionGeometry, InteractionKind,
    InteractionPattern, InteractionStyle,
};
pub use kinds::{
    CATEGORICAL_COLORS, ColorScheme, MAX_VOLUME_TRANSFER_POINTS, Representation,
    RepresentationKind, RepresentationParams, RepresentationPreset, RepresentationTarget,
    SECONDARY_STRUCTURE_COLORS, SurfaceKind, SurfaceStyle, VolumeRegion, VolumeRendering,
    VolumeSlice, VolumeStyle, VolumeTransferFunction, VolumeTransferPoint,
};
pub use material::{Material, MaterialModel};
pub use mesh::{FaceVisibility, MAX_MESH_VERTICES, Mesh, MeshTopology, MeshVertex};
pub use mesh_instance::MeshInstance;
pub use overlay::{OverlayAnchor, OverlayContent, ScreenOverlay};
pub use quadric::Quadric;
pub use radii::{cpk_color, vdw_radius};
pub use relation::{
    AnchorLayout, Relation, RelationBatch, RelationDependency, RelationLayout, RelationPartition,
    RelationPattern, RelationStyle, SpatialAnchor, TemplatePartRef,
};
pub use scalar::{ScalarContours, ScalarFieldSemantics, ScalarRamp, SurfaceScalarOverlay};
pub use surface_components::{
    SurfaceComponentPolicy, SurfaceComponentPolicyError, SurfaceComponentThreshold,
};
pub use tube_radius::TubeRadiusMapping;
pub use visual::{
    BoolExpr, ColorExpr, ColorParameter, MAX_VISUAL_INSTRUCTIONS, MAX_VISUAL_PARAMETERS,
    MAX_VISUAL_PROPERTIES, ScalarExpr, ScalarParameter, VectorExpr, VectorParameter,
    VisualAttributeRef, VisualColumnKey, VisualCompatibility, VisualEmitError, VisualError,
    VisualEvaluation, VisualInputs, VisualInstructionGpu, VisualOutput, VisualPipeline,
    VisualProgram, VisualProgramBuilder, VisualStage, VisualStyle, emit_resolve, is_specializable,
    is_supported, opcode_name, unsupported, visual_pipeline,
};
pub use visual_descriptor::VisualDescriptor;
