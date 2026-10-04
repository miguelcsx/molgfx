//! Stable identities of overlay domains.
use crate::{
    AnnotationId, EllipsoidId, InteractionId, MeasurementId, PlaneId, TrajectoryId, VolumeId,
};
use serde::{Deserialize, Serialize};
/// Identifies an overlay without conflating its independent domain counters.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "snake_case")]
pub enum OverlayId {
    /// Scalar density grid.
    Volume(VolumeId),
    /// Categorical label grid.
    Segmentation(crate::SegmentationId),
    /// Text annotation.
    Annotation(AnnotationId),
    /// Geometric measurement.
    Measurement(MeasurementId),
    /// Explicit molecular interaction.
    Interaction(InteractionId),
    /// Coordinate trajectory.
    Trajectory(TrajectoryId),
    /// Displacement tensor overlay.
    Ellipsoid(EllipsoidId),
    /// Guide plane.
    Plane(PlaneId),
}
