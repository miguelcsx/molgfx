//! Allocation-free synchronization of caller-decoded temporal streams.
//!
//! The timeline maps one global clock onto independently scaled local clocks.
//! It retains only the active two rows of a scalar stream and delegates decoded
//! coordinate residency to each structure's two-frame trajectory segment. A
//! tick costs `O(tracks + temporal property atoms)` and allocates nothing after
//! track setup.

use super::presentation::validate_presentation_time;
use crate::handle::SlotMap;
use crate::{
    AtomPropertyHandle, AttributeHandle, AttributeValues, CoreError, InstanceBatchHandle,
    PointBatchHandle, Scene, StructureHandle, TimelineTrackHandle,
};
use std::sync::Arc;

#[cfg(test)]
#[path = "timeline_tests.rs"]
mod tests;

/// How a local clock behaves after reaching its declared interval boundary.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum PlaybackMode {
    /// Hold the first or last sample outside the interval.
    #[default]
    Clamp,
    /// Wrap from the end back to the beginning.
    Loop,
    /// Alternate forward and backward traversal without a discontinuity.
    PingPong,
}

/// Constant-rate mapping from the shared clock to one local interval.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct TimeWarp {
    global_origin: f64,
    local_origin: f64,
    rate: f64,
    range: [f64; 2],
    playback: PlaybackMode,
}

impl TimeWarp {
    /// Builds a finite affine clock mapping with an increasing local interval.
    ///
    /// A rate of zero freezes the track. Negative rates play it backwards.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidTimeline`] for non-finite parameters, an
    /// empty interval or a local origin outside that interval.
    pub fn new(
        global_origin: f64,
        local_origin: f64,
        rate: f64,
        range: [f64; 2],
        playback: PlaybackMode,
    ) -> Result<Self, CoreError> {
        if !global_origin.is_finite()
            || !local_origin.is_finite()
            || !rate.is_finite()
            || !range.into_iter().all(f64::is_finite)
            || range[0] >= range[1]
            || !(range[0]..=range[1]).contains(&local_origin)
        {
            return Err(invalid(
                "time warp values must be finite over an increasing interval",
            ));
        }
        Ok(Self {
            global_origin,
            local_origin,
            rate,
            range,
            playback,
        })
    }

    /// Maps one finite global timestamp to local time in constant time.
    #[must_use]
    pub fn sample(self, global_seconds: f64) -> Option<f64> {
        if !global_seconds.is_finite() {
            return None;
        }
        let raw = self.local_origin + (global_seconds - self.global_origin) * self.rate;
        if !raw.is_finite() {
            return None;
        }
        let [start, end] = self.range;
        let duration = end - start;
        Some(match self.playback {
            PlaybackMode::Clamp => raw.clamp(start, end),
            PlaybackMode::Loop => start + (raw - start).rem_euclid(duration),
            PlaybackMode::PingPong => {
                let phase = (raw - start).rem_euclid(duration * 2.0);
                if phase <= duration {
                    start + phase
                } else {
                    end - (phase - duration)
                }
            }
        })
    }

    /// Normalized local phase after applying the playback boundary policy.
    #[must_use]
    pub fn phase(self, global_seconds: f64) -> Option<f32> {
        let sample = self.sample(global_seconds)?;
        num_traits::cast((sample - self.range[0]) / (self.range[1] - self.range[0]))
    }

    /// Closed local interval addressed by this mapping.
    #[must_use]
    pub const fn range(self) -> [f64; 2] {
        self.range
    }
}

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

/// Independent temporal tracks driven by one caller-owned timestamp.
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
        if start < f64::from(segment.start().time_seconds())
            || end > f64::from(segment.end().time_seconds())
        {
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
        if start < f64::from(segment.start().time_seconds())
            || end > f64::from(segment.end().time_seconds())
        {
            return Err(invalid(
                "dynamic topology time warp escapes its resident segment",
            ));
        }
        Ok(TimelineTrackHandle(
            self.tracks
                .insert(TimelineTrack::BondTopology { structure, warp }),
        ))
    }

    /// Applies one global time atomically after validating every target.
    ///
    /// No scene state changes if any track is stale or cannot sample the given
    /// timestamp. Setup has already reserved temporal property memory, so a
    /// successful tick performs no heap allocation.
    ///
    /// # Errors
    ///
    /// Returns a typed error without changing the scene when time is invalid,
    /// a target is stale or an active resident interval no longer matches.
    pub fn apply(&mut self, scene: &mut Scene, global_seconds: f64) -> Result<(), CoreError> {
        if !global_seconds.is_finite() {
            return Err(invalid("global timeline time must be finite"));
        }
        let presentation_time = validate_presentation_time(global_seconds)?;
        for (_, track) in self.tracks.iter() {
            validate_track(scene, track, global_seconds)?;
        }
        for (_, track) in self.tracks.iter() {
            match track {
                TimelineTrack::Trajectory { structure, warp } => {
                    let local = trajectory_sample(*warp, global_seconds)?;
                    scene.set_trajectory_time(*structure, local)?;
                }
                TimelineTrack::BondTopology { structure, warp } => {
                    let local = trajectory_sample(*warp, global_seconds)?;
                    scene.set_bond_topology_time(*structure, local)?;
                }
                TimelineTrack::AtomProperty {
                    property,
                    start,
                    end,
                    warp,
                } => {
                    let alpha = warp
                        .phase(global_seconds)
                        .ok_or_else(|| invalid("timeline sample overflowed"))?;
                    scene.interpolate_atom_property(*property, start, end, alpha)?;
                }
                TimelineTrack::Instances { batch, warp } => {
                    let alpha = warp
                        .phase(global_seconds)
                        .ok_or_else(|| invalid("timeline sample overflowed"))?;
                    scene.sample_instance_frames(*batch, alpha)?;
                }
                TimelineTrack::Points { batch, warp } => {
                    let alpha = warp
                        .phase(global_seconds)
                        .ok_or_else(|| invalid("timeline sample overflowed"))?;
                    scene.sample_point_frames(*batch, alpha)?;
                }
                TimelineTrack::Attribute { attribute, warp } => {
                    let alpha = warp
                        .phase(global_seconds)
                        .ok_or_else(|| invalid("timeline sample overflowed"))?;
                    scene.sample_attribute_frames(*attribute, alpha)?;
                }
            }
        }
        scene.apply_presentation_time(presentation_time);
        self.time_seconds = global_seconds;
        self.revision = self.revision.wrapping_add(1);
        Ok(())
    }

    /// Most recently applied global timestamp.
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

fn validate_track(scene: &Scene, track: &TimelineTrack, global: f64) -> Result<(), CoreError> {
    match track {
        TimelineTrack::Trajectory { structure, warp } => {
            let sample = trajectory_sample(*warp, global)?;
            let placed = scene.structure(*structure).ok_or(CoreError::StaleHandle)?;
            let segment = placed
                .trajectory()
                .ok_or_else(|| invalid("trajectory track lost its resident segment"))?;
            if sample < segment.start().time_seconds() || sample > segment.end().time_seconds() {
                return Err(invalid("trajectory sample escapes its resident segment"));
            }
        }
        TimelineTrack::BondTopology { structure, warp } => {
            let sample = trajectory_sample(*warp, global)?;
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
            warp.phase(global)
                .ok_or_else(|| invalid("timeline sample overflowed"))?;
        }
        TimelineTrack::Instances { batch, warp } => {
            scene
                .instance_frames(*batch)
                .ok_or(CoreError::StaleHandle)?;
            warp.phase(global)
                .ok_or_else(|| invalid("timeline sample overflowed"))?;
        }
        TimelineTrack::Points { batch, warp } => {
            scene.point_frames(*batch).ok_or(CoreError::StaleHandle)?;
            warp.phase(global)
                .ok_or_else(|| invalid("timeline sample overflowed"))?;
        }
        TimelineTrack::Attribute { attribute, warp } => {
            scene
                .attribute_frames(*attribute)
                .ok_or(CoreError::StaleHandle)?;
            warp.phase(global)
                .ok_or_else(|| invalid("timeline sample overflowed"))?;
        }
    }
    Ok(())
}

fn trajectory_sample(warp: TimeWarp, global: f64) -> Result<f32, CoreError> {
    let sample = warp
        .sample(global)
        .ok_or_else(|| invalid("timeline sample overflowed"))?;
    num_traits::cast(sample).ok_or_else(|| invalid("trajectory time exceeds finite f32 range"))
}

const fn invalid(reason: &'static str) -> CoreError {
    CoreError::InvalidTimeline { reason }
}

include!("timeline_generic_tracks.rs");
