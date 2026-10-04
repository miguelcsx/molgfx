//! The overlay domains a local patch replaces.

use crate::{
    AnnotationId, AnnotationSpec, InteractionId, InteractionSpec, MeasurementId, MeasurementSpec,
    TrajectoryId, TrajectorySpec, VolumeId, VolumeSpec,
};
use std::collections::BTreeMap;

#[derive(Default)]
pub(super) struct OverlayDomains {
    pub(super) volumes: Option<BTreeMap<VolumeId, VolumeSpec>>,
    pub(super) segmentations: Option<BTreeMap<crate::SegmentationId, crate::SegmentationSpec>>,
    pub(super) annotations: Option<BTreeMap<AnnotationId, AnnotationSpec>>,
    pub(super) measurements: Option<BTreeMap<MeasurementId, MeasurementSpec>>,
    pub(super) interactions: Option<BTreeMap<InteractionId, InteractionSpec>>,
    pub(super) trajectories: Option<BTreeMap<TrajectoryId, TrajectorySpec>>,
    pub(super) ellipsoids: Option<BTreeMap<crate::EllipsoidId, crate::overlay::EllipsoidSpec>>,
    pub(super) planes: Option<BTreeMap<crate::PlaneId, crate::PlaneSpec>>,
}
