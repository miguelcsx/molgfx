use crate::{DataSource, MolecularSurfaceData, SceneSpec, StructureId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Complete authored state and scientific assets, with molecular sources supplied separately.
/// Resident coordinate columns are deduplicated; restored calculations and playback share them.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SceneArchive {
    pub(crate) scene: SceneSpec,
    pub(crate) retained: Vec<Self>,
    pub(crate) frames: Vec<Frame>,
    pub(crate) properties: BTreeMap<Box<str>, Vec<u32>>,
    pub(crate) volumes: BTreeMap<crate::VolumeId, Vec<f32>>,
    pub(crate) segmentations: BTreeMap<crate::SegmentationId, Vec<u32>>,
    pub(crate) hydrogen_bonds: BTreeMap<crate::InteractionId, Bonds>,
    pub(crate) surfaces: BTreeMap<crate::RepresentationId, Surface>,
    pub(crate) trajectories: BTreeMap<crate::TrajectoryId, Interval>,
    pub(crate) playback: BTreeMap<StructureId, Interval>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Frame {
    pub(crate) index: u64,
    pub(crate) time: f64,
    pub(crate) positions: Vec<[f32; 3]>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub(crate) struct Interval {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) sample: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Bonds {
    pub(crate) source: DataSource,
    pub(crate) structure: StructureId,
    pub(crate) rows: Vec<[u32; 3]>,
    pub(crate) candidate_distance: Option<f32>,
    pub(crate) interval: Option<Interval>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Surface {
    pub(crate) source: DataSource,
    pub(crate) structure: StructureId,
    pub(crate) data: MolecularSurfaceData,
    pub(crate) interval: Option<Interval>,
}

impl SceneArchive {
    /// Original authored document, including algorithm and numerical controls.
    #[must_use]
    pub const fn scene(&self) -> &SceneSpec {
        &self.scene
    }

    /// Authored snapshots whose scientific assets are retained by this archive.
    pub fn retained_scenes(&self) -> impl Iterator<Item = &SceneSpec> {
        self.retained.iter().map(|archive| &archive.scene)
    }

    /// Encodes deterministic scientific data; call on a worker for large geometry.
    /// # Errors
    /// Returns an error when the archive cannot be encoded.
    pub fn to_json(&self) -> Result<String, crate::Error> {
        Ok(serde_json::to_string(self)?)
    }

    /// Reads archive data without publishing it. [`Self::restore`] validates all assets.
    /// # Errors
    /// Refuses malformed encoded data.
    pub fn from_json(value: &str) -> Result<Self, crate::Error> {
        Ok(serde_json::from_str(value)?)
    }
}
