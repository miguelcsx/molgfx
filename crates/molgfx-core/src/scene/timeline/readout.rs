//! Typed per-track readout of the most recently applied playback time.

use super::{TimeWarp, Timeline};
use crate::{PlaybackTime, SimTime, TimeUnit, TimelineTrackHandle};

/// Where one track stands: its physical time, the unit to show it in, and its
/// normalized phase across its interval.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct TrackReading {
    /// The track's physical time after its boundary policy.
    pub time: SimTime,
    /// The unit a viewer reads `time` in.
    pub unit: TimeUnit,
    /// Fraction of the track's interval already played, in `[0, 1]`.
    pub phase: f64,
}

impl TrackReading {
    /// The physical time expressed in the track's own display unit.
    #[must_use]
    pub fn display_value(self) -> f64 {
        self.time.to_unit(self.unit)
    }

    fn of(warp: TimeWarp, playback_seconds: f64) -> Option<Self> {
        let sample = warp.sample(playback_seconds)?;
        Some(Self {
            time: SimTime::new(sample).ok()?,
            unit: warp.unit(),
            phase: warp.phase(playback_seconds)?,
        })
    }
}

impl Timeline {
    /// Most recently applied playback time as a typed presentation instant.
    #[must_use]
    pub fn playback_time(&self) -> PlaybackTime {
        PlaybackTime::from(self.time_seconds)
    }

    /// The warp a track was registered with, or `None` for a removed track.
    #[must_use]
    pub fn warp(&self, handle: TimelineTrackHandle) -> Option<TimeWarp> {
        self.tracks.get(handle.0).map(super::TimelineTrack::warp)
    }

    /// One track's reading at the most recently applied playback time.
    ///
    /// Constant time and allocation-free.
    #[must_use]
    pub fn reading(&self, handle: TimelineTrackHandle) -> Option<TrackReading> {
        TrackReading::of(self.warp(handle)?, self.time_seconds)
    }

    /// Every track's reading at the most recently applied playback time.
    ///
    /// Tracks the clock cannot read are skipped. The iterator borrows the
    /// timeline and allocates nothing.
    pub fn readings(&self) -> impl Iterator<Item = (TimelineTrackHandle, TrackReading)> + '_ {
        self.tracks.iter().filter_map(|(raw, track)| {
            TrackReading::of(track.warp(), self.time_seconds)
                .map(|reading| (TimelineTrackHandle(raw), reading))
        })
    }

    /// Handles of every active track, in registration order of storage.
    pub fn track_handles(&self) -> impl Iterator<Item = TimelineTrackHandle> + '_ {
        self.tracks.iter().map(|(raw, _)| TimelineTrackHandle(raw))
    }
}
