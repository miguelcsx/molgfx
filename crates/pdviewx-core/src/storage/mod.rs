//! Generic scene storage, handles and typed core errors.

pub(crate) mod column;
pub(crate) mod error;
pub(crate) mod handle;

pub use column::{Column, Revision};
pub use error::CoreError;
pub use handle::{
    AnnotationHandle, AtomPropertyHandle, EnsembleHandle, GuideHandle, InteractionHandle,
    MeasurementHandle, MeshHandle, MeshInstanceHandle, OverlayHandle, PrimitiveHandle,
    RepresentationHandle, SegmentationHandle, SelectionHandle, StructureHandle, VolumeHandle,
};
