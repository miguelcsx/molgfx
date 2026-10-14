//! Authored time: independently paced tracks driven by one playback clock.

use crate::Error;
use crate::id::TrajectoryId;
use molgfx_core::{PlaybackMode, SimTime, TimeUnit, TimeWarp};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[cfg(test)]
#[path = "timeline_spec_tests.rs"]
mod tests;

/// How a track's physical interval is paced against the playback clock.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TrackPace {
    /// Plays the whole interval once across this many playback seconds.
    Fit {
        /// Playback seconds the interval takes.
        playback_seconds: f64,
    },
    /// Plays at a fixed physical pace.
    Dilation {
        /// Physical time per playback second, read in the track's unit.
        per_playback_second: f64,
    },
}

/// One authored track: a trajectory, the physical interval it plays and its pace.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub struct TrackSpec {
    /// Trajectory declared in the same scene that this track drives.
    pub trajectory: TrajectoryId,
    /// Unit that `range` and any dilation pace are authored in and shown in.
    pub unit: TimeUnit,
    /// Closed physical interval, as `[start, end]` in `unit`.
    pub range: [f64; 2],
    /// Pace against the playback clock.
    pub pace: TrackPace,
    /// Behaviour past the interval boundary.
    pub mode: PlaybackMode,
}

impl TrackSpec {
    /// Lowers the authored values to the core clock mapping.
    ///
    /// # Errors
    ///
    /// Returns an error for a non-finite value, an empty interval or a pace
    /// the mapping rejects.
    pub fn warp(&self) -> Result<TimeWarp, Error> {
        let [start, end] = self.range.map(|value| SimTime::from_unit(value, self.unit));
        let range = [start?, end?];
        let warp = match self.pace {
            TrackPace::Fit { playback_seconds } => {
                TimeWarp::fit(range, playback_seconds, self.mode)?
            }
            TrackPace::Dilation {
                per_playback_second,
            } => TimeWarp::dilation(
                SimTime::from_unit(per_playback_second, self.unit)?,
                range,
                self.mode,
            )?,
        };
        Ok(warp.with_unit(self.unit))
    }
}

/// Named tracks in stable order, authored as part of a scene.
#[derive(Clone, Default, PartialEq, Debug, Serialize, Deserialize)]
pub struct TimelineSpec {
    /// Tracks by name.
    #[serde(default)]
    pub tracks: BTreeMap<Box<str>, TrackSpec>,
}

impl TimelineSpec {
    /// Whether no track is authored.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tracks.is_empty()
    }

    /// Checks every track against the scene's declared trajectories.
    pub(crate) fn validate(
        &self,
        trajectories: &BTreeMap<TrajectoryId, crate::TrajectorySpec>,
    ) -> Result<(), Error> {
        for (name, track) in &self.tracks {
            if !trajectories.contains_key(&track.trajectory) {
                return Err(Error::InvalidSpec(format!(
                    "timeline track `{name}` targets an undeclared trajectory"
                )));
            }
            track.warp()?;
        }
        Ok(())
    }
}
