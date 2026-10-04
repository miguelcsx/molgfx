//! Declarative overlay scene items whose bulk data stays in runtime bindings.

#[cfg(test)]
mod surfaces_tests;
#[cfg(test)]
mod tests;

pub(crate) mod bindings;
mod builders;
pub(crate) mod lower;
mod lower_guides;
#[cfg(test)]
mod lower_tests;
mod planes;
mod surfaces;
mod validation;
pub(crate) use bindings::OverlayBindings;
pub use bindings::{
    OverlayHandles, TrajectoryBinding, TrajectoryFrame, VolumeBinding, VolumeStatistics,
};
pub use builders::{annotation, density, ellipsoid, interaction, measurement, trajectory};
pub use planes::PlaneSpec;
pub use surfaces::{AssemblyInstance, AssemblySpec, FitResult, UnitCellSpec, ValidationFinding};

mod anchor;
mod data_source;
mod ellipsoid_spec;
mod identity;
mod interaction_spec;
mod item;
mod label_spec;
mod lower_volume;
mod measurement_spec;
mod trajectory_spec;
mod volume_presentation;
mod volume_spec;
pub use identity::OverlayId;

pub use anchor::Anchor;
pub use data_source::DataSource;
pub use ellipsoid_spec::EllipsoidSpec;
pub use interaction_spec::{InteractionKind, InteractionSpec};
pub use label_spec::{AnnotationSpec, Label};
pub use measurement_spec::{MeasurementSpec, measurement_anchors, measurement_shape};
pub use trajectory_spec::TrajectorySpec;
pub use volume_presentation::{IsoStyle, VolumePresentation, VolumeTransferPoint};
pub use volume_spec::{Volume, VolumeRegion, VolumeSpec};
mod lower_segmentation;
mod segmentation_binding;
pub(crate) mod segmentation_spec;
pub use segmentation_binding::SegmentationBinding;
pub use segmentation_spec::{SegmentStyle, SegmentationSpec};
