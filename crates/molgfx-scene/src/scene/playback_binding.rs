//! Retained imperative playback inputs restored outside the authored document.

use crate::{StructureId, TrajectoryFrame};

/// One topology-stable molecular instance and its resident interpolation interval.
/// Coordinates remain shared with their original frame owners.
#[derive(Clone, Debug)]
pub struct PlaybackBinding {
    /// Molecular placement receiving the interval.
    pub structure: StructureId,
    /// First retained coordinate column.
    pub start: TrajectoryFrame,
    /// Second retained coordinate column.
    pub end: TrajectoryFrame,
    /// Presentation sample within the closed interval.
    pub sample_seconds: f64,
}
