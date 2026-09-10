//! Generic scene storage, handles and typed core errors.

pub(crate) mod column;
pub(crate) mod error;
pub(crate) mod handle;

pub use column::{Column, Revision};
pub use error::CoreError;
pub use handle::{
    AnnotationHandle, AtomPropertyHandle, AttributeHandle, GuideHandle, InstanceBatchHandle,
    InteractionHandle, LigandPoseBatchHandle, MeasurementHandle, MeshHandle, MeshInstanceHandle,
    OverlayHandle, PointBatchHandle, PrimitiveHandle, RelationBatchHandle, RepresentationHandle,
    SegmentationHandle, SelectionHandle, StructureHandle, TimelineTrackHandle, VolumeHandle,
};
