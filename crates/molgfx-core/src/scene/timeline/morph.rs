//! A two-frame morph: one trajectory track between two chosen frames.

use super::{PlaybackMode, TimeWarp, Timeline, invalid};
use crate::{
    CoreError, PlaybackTime, Scene, SimTime, StructureHandle, TimelineTrackHandle, TrajectoryFrame,
    TrajectorySegment,
};

#[cfg(test)]
#[path = "morph_tests.rs"]
mod tests;

impl Timeline {
    /// Morphs one structure from `from_frame` to `to_frame` over
    /// `playback_duration` of playback.
    ///
    /// This is an ordinary trajectory track, not a new kind: the two frames
    /// become the structure's resident pair and the track's warp fits their
    /// physical interval onto the duration, held at both ends. Applying the
    /// timeline at playback zero shows `from_frame`, at the duration shows
    /// `to_frame`, and in between the linear blend the GPU already computes.
    /// Cross-fade colour or opacity alongside it by binding Attribute tracks
    /// over the same playback clock with [`Timeline::bind_attribute`].
    ///
    /// # Errors
    ///
    /// Returns a typed error for a stale structure, a structure that already
    /// has a trajectory track, frames that are not ordered in index and time,
    /// frames that do not match the topology, or a duration that is not finite
    /// and positive. The scene and the timeline are unchanged on failure.
    pub fn morph(
        &mut self,
        scene: &mut Scene,
        structure: StructureHandle,
        from_frame: TrajectoryFrame,
        to_frame: TrajectoryFrame,
        playback_duration: impl Into<PlaybackTime>,
    ) -> Result<TimelineTrackHandle, CoreError> {
        if self
            .tracks
            .iter()
            .any(|(_, track)| track.targets_trajectory(structure))
        {
            return Err(invalid("a structure may have only one timeline track"));
        }
        let warp = morph_warp(&from_frame, &to_frame, playback_duration.into())?;
        let start = SimTime::from(from_frame.time_seconds());
        let segment = TrajectorySegment::new(from_frame, to_frame, start)?;
        scene.set_trajectory_segment(structure, segment)?;
        self.bind_trajectory(scene, structure, warp)
    }

    /// Whether `structure` already has a trajectory track.
    #[must_use]
    pub fn has_trajectory_track(&self, structure: StructureHandle) -> bool {
        self.tracks
            .iter()
            .any(|(_, track)| track.targets_trajectory(structure))
    }
}

/// The warp a morph between two frames plays: their interval over `duration`.
fn morph_warp(
    from_frame: &TrajectoryFrame,
    to_frame: &TrajectoryFrame,
    playback_duration: PlaybackTime,
) -> Result<TimeWarp, CoreError> {
    TimeWarp::fit(
        [
            SimTime::from(from_frame.time_seconds()),
            SimTime::from(to_frame.time_seconds()),
        ],
        playback_duration,
        PlaybackMode::Clamp,
    )
}
