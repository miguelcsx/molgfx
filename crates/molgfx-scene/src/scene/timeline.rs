//! Driving a scene's trajectories from a multi-track timeline.

use super::state::Scene;
use crate::{Error, StructureId, TrajectoryFrame};
use molgfx_core::{PlaybackMode, PlaybackTime, SimTime, TimeWarp, Timeline, TimelineTrackHandle};

impl Scene {
    /// Registers one structure's resident trajectory as a timeline track.
    ///
    /// The warp's interval must lie inside the structure's resident frame pair.
    ///
    /// # Errors
    ///
    /// Returns an error for a structure this scene does not carry, one with no
    /// resident pair, an interval outside that pair, or a second track on the
    /// same structure.
    pub fn bind_timeline_trajectory(
        &self,
        timeline: &mut Timeline,
        structure: StructureId,
        warp: TimeWarp,
    ) -> Result<TimelineTrackHandle, Error> {
        let handle = self.structure_handle(structure).ok_or_else(|| {
            Error::InvalidSpec("timeline track targets an unbound structure".to_owned())
        })?;
        Ok(timeline.bind_trajectory(&self.resolved, handle, warp)?)
    }

    /// Morphs `structure` from one frame to another over `playback_duration`.
    ///
    /// The two frames become the structure's resident pair and a single
    /// trajectory track plays between them, held at both ends: playback zero
    /// shows `from_frame`, the duration shows `to_frame`, and the time between
    /// is the linear blend. Cross-fade colour or opacity over the same clock
    /// with Attribute tracks. Nothing changes if any check fails.
    ///
    /// # Errors
    ///
    /// Returns an error for a structure this scene does not carry, one that
    /// already has a trajectory track, frames that are unordered or do not
    /// match the structure, or a duration that is not finite and positive.
    pub fn morph_timeline_trajectory(
        &mut self,
        timeline: &mut Timeline,
        structure: StructureId,
        from_frame: TrajectoryFrame,
        to_frame: TrajectoryFrame,
        playback_duration: impl Into<PlaybackTime>,
    ) -> Result<TimelineTrackHandle, Error> {
        let handle = self.structure_handle(structure).ok_or_else(|| {
            Error::InvalidSpec("timeline track targets an unbound structure".to_owned())
        })?;
        if timeline.has_trajectory_track(handle) {
            return Err(Error::InvalidSpec(
                "a structure may have only one timeline track".to_owned(),
            ));
        }
        let warp = TimeWarp::fit(
            [
                SimTime::from(from_frame.time_seconds()),
                SimTime::from(to_frame.time_seconds()),
            ],
            playback_duration,
            PlaybackMode::Clamp,
        )?;
        let start = from_frame.time_seconds();
        self.set_trajectory_pair(structure, from_frame, to_frame, start)?;
        Ok(timeline.bind_trajectory(&self.resolved, handle, warp)?)
    }

    /// Applies one playback time to every track of `timeline`, atomically.
    ///
    /// The sample times are retained so a later resolution of the scene keeps
    /// the animation where it is. A tick allocates nothing.
    ///
    /// # Errors
    ///
    /// Returns an error without changing the scene when the time is invalid or
    /// a track no longer matches its resident interval.
    pub fn apply_timeline(
        &mut self,
        timeline: &mut Timeline,
        playback: impl Into<PlaybackTime>,
    ) -> Result<(), Error> {
        timeline.apply(&mut self.resolved, playback)?;
        let structures = self.structures.keys().copied();
        let handles = self.resolved.structures().map(|(handle, _)| handle);
        for (id, handle) in structures.zip(handles) {
            let Some(segment) = self
                .resolved
                .structure(handle)
                .and_then(molgfx_core::PlacedStructure::trajectory)
            else {
                continue;
            };
            if let Some(pair) = self.playback.get_mut(&id) {
                pair.sample_seconds = segment.sample_seconds();
            } else {
                self.overlay_bindings.retain_trajectory_time(segment);
            }
        }
        Ok(())
    }

    /// Lowers the authored timeline to a core [`Timeline`] over this scene.
    ///
    /// Each track binds the structure of its declared trajectory, whose
    /// resident frame pair must already cover the track's interval.
    ///
    /// # Errors
    ///
    /// Returns an error for a track with an invalid interval or pace, an
    /// undeclared trajectory, or a structure that cannot play it.
    pub fn lower_timeline(&self) -> Result<Timeline, Error> {
        let mut timeline = Timeline::new();
        for (name, track) in &self.spec.timeline.tracks {
            let structure = self
                .spec
                .trajectories
                .get(&track.trajectory)
                .map(|trajectory| trajectory.structure)
                .ok_or_else(|| {
                    Error::InvalidSpec(format!(
                        "timeline track `{name}` targets an undeclared trajectory"
                    ))
                })?;
            self.bind_timeline_trajectory(&mut timeline, structure, track.warp()?)?;
        }
        Ok(timeline)
    }
}
