//! Allocation-free synchronization of caller-decoded temporal streams.
//!
//! The timeline maps one playback clock onto independently scaled physical
//! clocks. Each track's [`TimeWarp`] converts playback seconds into its own
//! interval, so a femtosecond vibration and a millisecond domain motion can
//! run side by side. The timeline retains only the active two rows of a scalar
//! stream and delegates decoded coordinate residency to each structure's
//! two-frame trajectory segment. A tick costs `O(tracks + temporal property
//! atoms)` and allocates nothing after track setup.

use super::presentation::validate_presentation_time;
use crate::handle::SlotMap;
use crate::time::narrow_to_f32;
use crate::{
    AtomPropertyHandle, AttributeHandle, CoreError, InstanceBatchHandle, PlaybackTime,
    PointBatchHandle, Scene, StructureHandle, TimelineTrackHandle,
};
use std::sync::Arc;

mod generic_tracks;
mod morph;
mod readout;
mod time_warp;

pub use readout::TrackReading;
pub use time_warp::{PlaybackMode, TimeWarp};

#[cfg(test)]
#[path = "timeline_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "timeline/track_clock_tests.rs"]
mod track_clock_tests;

#[derive(Clone, Debug)]
enum TimelineTrack {
    Trajectory {
        structure: StructureHandle,
        warp: TimeWarp,
    },
    BondTopology {
        structure: StructureHandle,
        warp: TimeWarp,
    },
    AtomProperty {
        property: AtomPropertyHandle,
        start: Arc<[f32]>,
        end: Arc<[f32]>,
        warp: TimeWarp,
    },
    Instances {
        batch: InstanceBatchHandle,
        warp: TimeWarp,
    },
    Points {
        batch: PointBatchHandle,
        warp: TimeWarp,
    },
    Attribute {
        attribute: AttributeHandle,
        warp: TimeWarp,
    },
}

impl TimelineTrack {
    const fn warp(&self) -> TimeWarp {
        match self {
            Self::Trajectory { warp, .. }
            | Self::BondTopology { warp, .. }
            | Self::AtomProperty { warp, .. }
            | Self::Instances { warp, .. }
            | Self::Points { warp, .. }
            | Self::Attribute { warp, .. } => *warp,
        }
    }

    fn targets_trajectory(&self, candidate: StructureHandle) -> bool {
        matches!(self, Self::Trajectory { structure, .. } if *structure == candidate)
    }

    fn targets_bond_topology(&self, candidate: StructureHandle) -> bool {
        matches!(self, Self::BondTopology { structure, .. } if *structure == candidate)
    }

    fn targets_property(&self, candidate: AtomPropertyHandle) -> bool {
        matches!(self, Self::AtomProperty { property, .. } if *property == candidate)
    }

    fn targets_instances(&self, candidate: InstanceBatchHandle) -> bool {
        matches!(self, Self::Instances { batch, .. } if *batch == candidate)
    }

    fn targets_points(&self, candidate: PointBatchHandle) -> bool {
        matches!(self, Self::Points { batch, .. } if *batch == candidate)
    }

    fn targets_attribute(&self, candidate: AttributeHandle) -> bool {
        matches!(self, Self::Attribute { attribute, .. } if *attribute == candidate)
    }
}

/// Independent temporal tracks driven by one caller-owned playback clock.
#[derive(Clone, Debug, Default)]
pub struct Timeline {
    tracks: SlotMap<TimelineTrack>,
    time_seconds: f64,
    revision: u64,
}

impl Timeline {
    /// Creates an empty timeline at zero seconds.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds one independently warped structure trajectory.
    ///
    /// # Errors
    ///
    /// Returns a typed error for a stale structure, missing active segment,
    /// range outside that segment or a duplicate target.
    pub fn bind_trajectory(
        &mut self,
        scene: &Scene,
        structure: StructureHandle,
        warp: TimeWarp,
    ) -> Result<TimelineTrackHandle, CoreError> {
        if self
            .tracks
            .iter()
            .any(|(_, track)| track.targets_trajectory(structure))
        {
            return Err(invalid("a structure may have only one timeline track"));
        }
        let placed = scene.structure(structure).ok_or(CoreError::StaleHandle)?;
        let segment = placed
            .trajectory()
            .ok_or_else(|| invalid("trajectory track requires an active two-frame segment"))?;
        let [start, end] = warp.range();
        if start < segment.start().time_seconds() || end > segment.end().time_seconds() {
            return Err(invalid("trajectory time warp escapes its resident segment"));
        }
        Ok(TimelineTrackHandle(
            self.tracks
                .insert(TimelineTrack::Trajectory { structure, warp }),
        ))
    }

    /// Adds two active scalar rows and reserves their result storage once.
    ///
    /// # Errors
    ///
    /// Returns a typed error for a stale property, malformed rows or duplicate
    /// target. The rows remain shared with the caller.
    pub fn bind_atom_property(
        &mut self,
        scene: &mut Scene,
        property: AtomPropertyHandle,
        start: Arc<[f32]>,
        end: Arc<[f32]>,
        warp: TimeWarp,
    ) -> Result<TimelineTrackHandle, CoreError> {
        if self
            .tracks
            .iter()
            .any(|(_, track)| track.targets_property(property))
        {
            return Err(invalid("an atom property may have only one timeline track"));
        }
        if start.len() != end.len() || start.is_empty() {
            return Err(invalid(
                "temporal property rows must be non-empty and equal length",
            ));
        }
        let stored = scene
            .properties
            .get_mut(property.0)
            .ok_or(CoreError::StaleHandle)?;
        if stored.value.values().len() != start.len() {
            return Err(invalid("temporal property row does not match its topology"));
        }
        if start
            .iter()
            .chain(end.iter())
            .any(|value| value.is_infinite())
        {
            return Err(invalid("temporal property rows contain infinity"));
        }
        if !start
            .iter()
            .zip(end.iter())
            .any(|(&a, &b)| a.is_finite() && b.is_finite())
        {
            return Err(invalid(
                "temporal property rows require at least one finite pair",
            ));
        }
        stored.value.reserve_interpolation();
        Ok(TimelineTrackHandle(self.tracks.insert(
            TimelineTrack::AtomProperty {
                property,
                start,
                end,
                warp,
            },
        )))
    }

    /// Adds one independently warped dynamic covalent-topology stream.
    ///
    /// # Errors
    ///
    /// Returns a typed error for a stale structure, missing active segment,
    /// range outside that segment or a duplicate topology target.
    pub fn bind_bond_topology(
        &mut self,
        scene: &Scene,
        structure: StructureHandle,
        warp: TimeWarp,
    ) -> Result<TimelineTrackHandle, CoreError> {
        if self
            .tracks
            .iter()
            .any(|(_, track)| track.targets_bond_topology(structure))
        {
            return Err(invalid(
                "a structure may have only one dynamic topology track",
            ));
        }
        let placed = scene.structure(structure).ok_or(CoreError::StaleHandle)?;
        let segment = placed.bond_topology().ok_or_else(|| {
            invalid("dynamic topology track requires an active two-frame segment")
        })?;
        let [start, end] = warp.range();
        if start < segment.start().time_seconds() || end > segment.end().time_seconds() {
            return Err(invalid(
                "dynamic topology time warp escapes its resident segment",
            ));
        }
        Ok(TimelineTrackHandle(
            self.tracks
                .insert(TimelineTrack::BondTopology { structure, warp }),
        ))
    }

    /// Applies one playback time atomically after validating every target.
    ///
    /// No scene state changes if any track is stale or cannot sample the given
    /// time. Each track maps the playback time into its own physical interval in
    /// `f64`; only the blend fractions handed to the GPU or the CPU blends are
    /// narrowed to `f32`. Setup has already reserved temporal property memory,
    /// so a successful tick performs no heap allocation.
    ///
    /// # Errors
    ///
    /// Returns a typed error without changing the scene when time is invalid,
    /// a target is stale or an active resident interval no longer matches.
    pub fn apply(
        &mut self,
        scene: &mut Scene,
        playback: impl Into<PlaybackTime>,
    ) -> Result<(), CoreError> {
        let playback_seconds = playback.into().seconds();
        if !playback_seconds.is_finite() {
            return Err(invalid("global timeline time must be finite"));
        }
        let presentation_time = validate_presentation_time(playback_seconds)?;
        for (_, track) in self.tracks.iter() {
            validate_track(scene, track, playback_seconds)?;
        }
        for (_, track) in self.tracks.iter() {
            match track {
                TimelineTrack::Trajectory { structure, warp } => {
                    let local = physical_sample(*warp, playback_seconds)?;
                    scene.set_trajectory_time(*structure, local)?;
                    let clock = warp
                        .track_clock(playback_seconds)
                        .ok_or_else(|| invalid("timeline sample overflowed"))?;
                    scene.set_structure_track_clock(*structure, clock)?;
                }
                TimelineTrack::BondTopology { structure, warp } => {
                    let local = physical_sample(*warp, playback_seconds)?;
                    scene.set_bond_topology_time(*structure, local)?;
                }
                TimelineTrack::AtomProperty {
                    property,
                    start,
                    end,
                    warp,
                } => {
                    let alpha = blend_fraction(*warp, playback_seconds)?;
                    scene.interpolate_atom_property(*property, start, end, alpha)?;
                }
                TimelineTrack::Instances { batch, warp } => {
                    let alpha = blend_fraction(*warp, playback_seconds)?;
                    scene.sample_instance_frames(*batch, alpha)?;
                }
                TimelineTrack::Points { batch, warp } => {
                    let alpha = blend_fraction(*warp, playback_seconds)?;
                    scene.sample_point_frames(*batch, alpha)?;
                }
                TimelineTrack::Attribute { attribute, warp } => {
                    let alpha = blend_fraction(*warp, playback_seconds)?;
                    scene.sample_attribute_frames(*attribute, alpha)?;
                }
            }
        }
        scene.apply_presentation_time(presentation_time);
        self.time_seconds = playback_seconds;
        self.revision = self.revision.wrapping_add(1);
        Ok(())
    }

    /// Most recently applied playback time, in seconds.
    #[must_use]
    pub const fn time_seconds(&self) -> f64 {
        self.time_seconds
    }

    /// Number of active independently warped tracks.
    #[must_use]
    pub fn len(&self) -> usize {
        self.tracks.len()
    }

    /// Whether no temporal target is bound.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tracks.len() == 0
    }

    /// Revision bumped by each successful tick or removal.
    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }
}

fn validate_track(scene: &Scene, track: &TimelineTrack, playback: f64) -> Result<(), CoreError> {
    match track {
        TimelineTrack::Trajectory { structure, warp } => {
            let sample = physical_sample(*warp, playback)?;
            let placed = scene.structure(*structure).ok_or(CoreError::StaleHandle)?;
            let segment = placed
                .trajectory()
                .ok_or_else(|| invalid("trajectory track lost its resident segment"))?;
            if sample < segment.start().time_seconds() || sample > segment.end().time_seconds() {
                return Err(invalid("trajectory sample escapes its resident segment"));
            }
        }
        TimelineTrack::BondTopology { structure, warp } => {
            let sample = physical_sample(*warp, playback)?;
            let placed = scene.structure(*structure).ok_or(CoreError::StaleHandle)?;
            let segment = placed
                .bond_topology()
                .ok_or_else(|| invalid("dynamic topology track lost its resident segment"))?;
            if sample < segment.start().time_seconds() || sample > segment.end().time_seconds() {
                return Err(invalid(
                    "dynamic topology sample escapes its resident segment",
                ));
            }
        }
        TimelineTrack::AtomProperty {
            property,
            start,
            end,
            warp,
        } => {
            let value = scene
                .atom_property(*property)
                .ok_or(CoreError::StaleHandle)?;
            if value.values().len() != start.len() || start.len() != end.len() {
                return Err(invalid("temporal property topology changed"));
            }
            blend_fraction(*warp, playback)?;
        }
        TimelineTrack::Instances { batch, warp } => {
            scene
                .instance_frames(*batch)
                .ok_or(CoreError::StaleHandle)?;
            blend_fraction(*warp, playback)?;
        }
        TimelineTrack::Points { batch, warp } => {
            scene.point_frames(*batch).ok_or(CoreError::StaleHandle)?;
            blend_fraction(*warp, playback)?;
        }
        TimelineTrack::Attribute { attribute, warp } => {
            scene
                .attribute_frames(*attribute)
                .ok_or(CoreError::StaleHandle)?;
            blend_fraction(*warp, playback)?;
        }
    }
    Ok(())
}

/// The track's physical time in seconds, kept in `f64` for resident-interval checks.
fn physical_sample(warp: TimeWarp, playback: f64) -> Result<f64, CoreError> {
    warp.sample(playback)
        .ok_or_else(|| invalid("timeline sample overflowed"))
}

/// The track's normalized phase, narrowed to the `f32` a CPU or GPU blend consumes.
fn blend_fraction(warp: TimeWarp, playback: f64) -> Result<f32, CoreError> {
    warp.phase(playback)
        .and_then(narrow_to_f32)
        .ok_or_else(|| invalid("timeline sample overflowed"))
}

const fn invalid(reason: &'static str) -> CoreError {
    CoreError::InvalidTimeline { reason }
}
