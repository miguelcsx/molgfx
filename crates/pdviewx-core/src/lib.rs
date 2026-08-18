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
mod gpu;
mod representation;
mod scene;
mod selection;
mod serialization;
mod storage;
mod structure;

pub(crate) use controls::input;
pub(crate) use gpu::gpu_types;
pub(crate) use representation::{annotation, interaction, radii};
pub(crate) use selection::select;
pub(crate) use storage::{column, error, handle};
pub(crate) use structure::{atoms, coord, density, hierarchy, placed};

pub use controls::{ArcballController, Button, FlyController, InputEvent, Key, OrbitController};
pub use gpu::{
    AtomFlags, AtomGpu, BondGpu, DrawIndirectArgs, EntityId, EntityKind, EntityRef, InteractionGpu,
    ParticleMotionGpu, PrimitiveGpu, VolumeSegmentRef,
};
pub use representation::{
    Annotation, AnnotationAnchor, AnnotationKind, AtomProperty, AtomPropertyMeaning, ColorScheme,
    FaceVisibility, Guide, GuideCap, GuideStyle, InteractionAnchor, InteractionDirection,
    InteractionEdge, InteractionGeometry, InteractionKind, InteractionPattern, InteractionStyle,
    MAX_MESH_VERTICES, MAX_VOLUME_TRANSFER_POINTS, MarkerShape, MarkerStyle, Material,
    MaterialModel, Measurement, MeasurementKind, Mesh, MeshInstance, MeshTopology, MeshVertex,
    OverlayAnchor, OverlayContent, PolylineKind, PropertyAppearance, PropertyAppearanceSample,
    PropertyLegend, Quadric, Representation, RepresentationKind, RepresentationParams,
    RepresentationPreset, RepresentationTarget, ScalarContours, ScalarFieldSemantics, ScalarRamp,
    ScreenOverlay, SurfaceComponentPolicy, SurfaceKind, SurfaceScalarOverlay, SurfaceStyle,
    TubeRadiusMapping, VolumeRegion, VolumeRendering, VolumeSlice, VolumeStyle,
    VolumeTransferFunction, VolumeTransferPoint, cpk_color, vdw_radius,
};
pub use scene::Scene;
pub use selection::{
    AtomSelection, ClipCap, ClipPlane, ClipSet, MAX_CLIP_PLANES, PropertyComparison, Select,
};
pub use serialization::{
    AnchorDescription, AnnotationDescription, AtomPropertyDescription, ClipDescription,
    ColorDescription, EntityDescription, GuideDescription, GuideStyleDescription,
    InteractionDescription, MarkerStyleDescription, MaterialDescription, MeasurementDescription,
    MeshDescription, MeshInstanceDescription, ObjectIdentity, OverlayDescription,
    ParticleMotionDescription, PrimitiveDescription, PropertyAppearanceDescription,
    RegionDescription, RepresentationDescription, ScalarSemanticsDescription, SceneDescription,
    SceneDescriptionSources, SceneManifest, SegmentStyleDescription, SegmentationStyleDescription,
    SelectionDescription, SelectionMask, StructureDescription, SurfaceScalarDescription,
    TableCounts, TargetDescription, VolumeDescription, VolumeStyleDescription,
    VolumeTransferPointDescription,
};
pub use storage::{
    AnnotationHandle, AtomPropertyHandle, Column, CoreError, EnsembleHandle, GuideHandle,
    InteractionHandle, MeasurementHandle, MeshHandle, MeshInstanceHandle, OverlayHandle,
    PrimitiveHandle, RepresentationHandle, Revision, SegmentationHandle, SelectionHandle,
    StructureHandle, VolumeHandle,
};
pub use structure::{
    AnisotropicEllipsoid, AtomTable, CarbohydrateShape, CarbohydrateSymbol, CoordRef, CrystalCell,
    DensityVolume, Ensemble, EntityProvenance, Hierarchy, Particle, ParticleBoundary,
    ParticleMotion, ParticleShape, PlacedStructure, PlanarRegion, Primitive, ProvenanceDetail,
    SecondaryStructure, SegmentStyle, SegmentStyleTable, SegmentationStyle, SegmentedVolume,
    SymmetryInstance, TrajectoryFrame, TrajectorySegment, ValidationKind, ValidationMarker,
};
