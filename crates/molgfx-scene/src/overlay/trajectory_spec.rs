//! Trajectory metadata.

use super::DataSource;
use crate::id::StructureId;
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
    /// Optional positive interval between frames.
    pub time_step: Option<f64>,
    /// Optional physical unit for `time_step`.
    pub time_unit: Option<Box<str>>,
}
