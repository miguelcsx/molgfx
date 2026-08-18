//! Declarative visual components and caller-authored render annotations.

pub(crate) mod annotation;
pub(crate) mod atom_property;
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
pub(crate) mod scalar;
pub(crate) mod tube_radius;

pub use annotation::{
    Annotation, AnnotationAnchor, AnnotationKind, MarkerShape, MarkerStyle, Measurement,
    MeasurementKind,
};
pub use atom_property::{
    AtomProperty, AtomPropertyMeaning, PropertyAppearance, PropertyAppearanceSample, PropertyLegend,
};
pub use guide::{Guide, GuideCap, GuideStyle, PolylineKind};
pub use interaction::{
    InteractionAnchor, InteractionDirection, InteractionEdge, InteractionGeometry, InteractionKind,
    InteractionPattern, InteractionStyle,
};
pub use kinds::{
    ColorScheme, MAX_VOLUME_TRANSFER_POINTS, Representation, RepresentationKind,
    RepresentationParams, RepresentationPreset, RepresentationTarget, SurfaceKind, SurfaceStyle,
    VolumeRegion, VolumeRendering, VolumeSlice, VolumeStyle, VolumeTransferFunction,
    VolumeTransferPoint,
};
pub use material::{Material, MaterialModel};
pub use mesh::{
    FaceVisibility, MAX_MESH_VERTICES, Mesh, MeshTopology, MeshVertex, SurfaceComponentPolicy,
};
pub use mesh_instance::MeshInstance;
pub use overlay::{OverlayAnchor, OverlayContent, ScreenOverlay};
pub use quadric::Quadric;
pub use radii::{cpk_color, vdw_radius};
pub use scalar::{ScalarContours, ScalarFieldSemantics, ScalarRamp, SurfaceScalarOverlay};
pub use tube_radius::TubeRadiusMapping;
