//! Constant-rate mapping from the playback clock onto one physical interval.

use super::invalid;
use crate::{CoreError, PlaybackTime, SimTime, TimeUnit, TrackClock};
use serde::{Deserialize, Serialize};

/// How a local clock behaves after reaching its declared interval boundary.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackMode {
    /// Hold the first or last sample outside the interval.
    #[default]
    Clamp,
    /// Wrap from the end back to the beginning.
    Loop,
    /// Alternate forward and backward traversal without a discontinuity.
    PingPong,
}

/// Constant-rate mapping from the playback clock to one physical interval.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct TimeWarp {
    playback_origin: f64,
    local_origin: f64,
    rate: f64,
    range: [f64; 2],
    playback: PlaybackMode,
    unit: TimeUnit,
}

impl TimeWarp {
    /// Builds a finite affine clock mapping with an increasing local interval.
    ///
    /// A rate of zero freezes the track. Negative rates play it backwards.
    /// The display unit defaults to the one that fits the interval's length,
    /// so a track's `track_time` stays `f32`-safe unless a caller overrides it
    /// with [`with_unit`](Self::with_unit).
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidTimeline`] for non-finite parameters, an
    /// empty interval or a local origin outside that interval.
    pub fn new(
        playback_origin: f64,
        local_origin: f64,
        rate: f64,
        range: [f64; 2],
        playback: PlaybackMode,
    ) -> Result<Self, CoreError> {
        if !playback_origin.is_finite()
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
            playback_origin,
            local_origin,
            rate,
            range,
            playback,
            unit: TimeUnit::fitting(range[1] - range[0]),
        })
    }

    /// Reads this track's time in `unit` when a viewer displays it.
    ///
    /// The unit never changes the mapping, which stays in canonical seconds.
    #[must_use]
    pub const fn with_unit(mut self, unit: TimeUnit) -> Self {
        self.unit = unit;
        self
    }

    /// The unit this track's time is displayed in.
    #[must_use]
    pub const fn unit(self) -> TimeUnit {
        self.unit
    }

    /// Plays a physical interval at a fixed pace from its first sample.
    ///
    /// `sim_per_playback_second` is the physical time that elapses for each
    /// second of playback: one nanosecond per second shows a nanosecond of motion
    /// every playback second.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidTimeline`] for a non-finite pace or an empty
    /// interval.
    pub fn dilation(
        sim_per_playback_second: SimTime,
        range: [SimTime; 2],
        playback: PlaybackMode,
    ) -> Result<Self, CoreError> {
        let [start, end] = range.map(SimTime::seconds);
        Self::new(
            0.0,
            start,
            sim_per_playback_second.seconds(),
            [start, end],
            playback,
        )
        .map(|warp| warp.with_unit(TimeUnit::fitting(end - start)))
    }

    /// Plays a physical interval once across `playback_seconds`.
    ///
    /// The rate is derived from the two lengths, so a femtosecond vibration and
    /// a nanosecond drift can share one playback duration, each at its own pace.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidTimeline`] for an empty interval or a playback
    /// duration that is not finite and positive.
    pub fn fit(
        sim_range: [SimTime; 2],
        playback_seconds: impl Into<PlaybackTime>,
        playback: PlaybackMode,
    ) -> Result<Self, CoreError> {
        let [start, end] = sim_range.map(SimTime::seconds);
        let duration = playback_seconds.into().seconds();
        if !duration.is_finite() || duration <= 0.0 {
            return Err(invalid("playback duration must be finite and positive"));
        }
        Self::new(0.0, start, (end - start) / duration, [start, end], playback)
            .map(|warp| warp.with_unit(TimeUnit::fitting(end - start)))
    }

    /// Maps one finite playback timestamp to physical time in constant time.
    #[must_use]
    pub fn sample(self, playback_seconds: f64) -> Option<f64> {
        if !playback_seconds.is_finite() {
            return None;
        }
        let raw = self.local_origin + (playback_seconds - self.playback_origin) * self.rate;
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

    /// Normalized local phase in `[0, 1]` after applying the boundary policy.
    #[must_use]
    pub fn phase(self, playback_seconds: f64) -> Option<f64> {
        let sample = self.sample(playback_seconds)?;
        Some((sample - self.range[0]) / (self.range[1] - self.range[0]))
    }

    /// This track's phase and display-unit time at one playback timestamp.
    ///
    /// Both come from a single sample, kept in `f64`.
    #[must_use]
    pub fn track_clock(self, playback_seconds: f64) -> Option<TrackClock> {
        let sample = self.sample(playback_seconds)?;
        Some(TrackClock {
            phase: (sample - self.range[0]) / (self.range[1] - self.range[0]),
            display_time: sample / self.unit.seconds(),
        })
    }

    /// Physical seconds that elapse per second of playback: the track's
    /// temporal dilation. Negative plays backwards; zero freezes the track.
    #[must_use]
    pub const fn rate(self) -> f64 {
        self.rate
    }

    /// Closed local interval addressed by this mapping, in seconds.
    #[must_use]
    pub const fn range(self) -> [f64; 2] {
        self.range
    }
}
