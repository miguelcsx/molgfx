//! Trajectory metadata.

use super::DataSource;
use crate::id::StructureId;
use molgfx_core::{SimTime, TimeUnit};
use serde::{Deserialize, Serialize};

/// Trajectory metadata. Frames are supplied by a runtime binding or data source.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct TrajectorySpec {
    /// Structure whose topology owns each frame.
    pub structure: StructureId,
    /// Portable source descriptor for frame data.
    pub source: DataSource,
    /// Declared number of frames.
    pub frame_count: u64,
    /// Optional positive physical interval between frames.
    pub time_step: Option<SimTime>,
    /// Optional unit the step was authored in and is shown in.
    pub time_unit: Option<TimeUnit>,
}
