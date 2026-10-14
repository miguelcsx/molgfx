//! The schedule of a deterministic timeline sequence and its sidecar manifest.

use crate::Error;
use molgfx_core::{Timeline, TimelineTrackHandle};
use molgfx_render::MeasuredOutput;
use num_traits::ToPrimitive as _;
use serde_json::{Value, json};
use std::ops::Range;

#[cfg(test)]
#[path = "timeline_sequence_tests.rs"]
mod tests;

/// What the manifest says about every sequence, verbatim.
const CAPTION: &str = "times are file timestamps; between saved frames the motion is linear interpolation, not simulation";

/// Which frames of a sequence to render, and how.
///
/// Frame `i` is shown at exactly `i / fps` seconds of playback, computed in
/// `f64` from the index, never by adding a step.
#[derive(Clone, PartialEq, Debug)]
pub struct FrameSchedule {
    /// Playback frames per second: finite and positive.
    pub fps: f64,
    /// Frame indices to render, `start` inclusive and `end` exclusive.
    pub range: Range<u64>,
    /// The exposure of every frame. `Converged` averages the tier's full
    /// sample count; `Interactive` is one single-sample frame.
    pub quality: MeasuredOutput,
}

impl FrameSchedule {
    /// A converged schedule over `range` at `fps`.
    #[must_use]
    pub const fn converged(fps: f64, range: Range<u64>) -> Self {
        Self {
            fps,
            range,
            quality: MeasuredOutput::Converged,
        }
    }

    /// Checks the schedule and returns its frame count.
    ///
    /// # Errors
    ///
    /// Returns an invalid-specification error for a non-finite or non-positive
    /// frame rate, or for a range that holds no frame.
    pub fn validate(&self) -> Result<usize, Error> {
        if !self.fps.is_finite() || self.fps <= 0.0 {
            return Err(Error::InvalidSpec(
                "sequence frame rate must be finite and positive".to_owned(),
            ));
        }
        if self.range.end <= self.range.start {
            return Err(Error::InvalidSpec(
                "sequence frame range must hold at least one frame".to_owned(),
            ));
        }
        (self.range.end - self.range.start)
            .to_usize()
            .ok_or_else(|| {
                Error::InvalidSpec("sequence frame count is not representable".to_owned())
            })
    }

    /// Playback seconds of frame `index`, exactly `index / fps`.
    ///
    /// # Errors
    ///
    /// Returns an invalid-specification error when the index or the quotient is
    /// not representable.
    pub fn playback_seconds(&self, index: u64) -> Result<f64, Error> {
        let seconds = index
            .to_f64()
            .map(|index| index / self.fps)
            .filter(|seconds| seconds.is_finite());
        seconds.ok_or_else(|| Error::InvalidSpec("frame time is not representable".to_owned()))
    }
}

/// The per-sequence record written beside the frames.
///
/// Track descriptions are kept once and per-frame sim times in one flat
/// column, so recording a frame allocates nothing after construction.
#[derive(Clone, Debug)]
pub struct SequenceManifest {
    schedule: FrameSchedule,
    handles: Vec<TimelineTrackHandle>,
    units: Vec<&'static str>,
    dilations: Vec<f64>,
    playback_seconds: Vec<f64>,
    sim_seconds: Vec<f64>,
}

impl SequenceManifest {
    /// Opens a manifest for `schedule`, reserving room for every frame.
    ///
    /// # Errors
    ///
    /// Returns an invalid-specification error for an invalid schedule or a
    /// track that no longer has a warp.
    pub fn new(schedule: &FrameSchedule, timeline: &Timeline) -> Result<Self, Error> {
        let frames = schedule.validate()?;
        let handles: Vec<TimelineTrackHandle> = timeline.track_handles().collect();
        let mut units = Vec::with_capacity(handles.len());
        let mut dilations = Vec::with_capacity(handles.len());
        for handle in &handles {
            let warp = timeline
                .warp(*handle)
                .ok_or_else(|| Error::InvalidSpec("timeline track has no warp".to_owned()))?;
            units.push(warp.unit().symbol());
            dilations.push(warp.rate());
        }
        let columns = frames.saturating_mul(handles.len());
        Ok(Self {
            schedule: schedule.clone(),
            handles,
            units,
            dilations,
            playback_seconds: Vec::with_capacity(frames),
            sim_seconds: Vec::with_capacity(columns),
        })
    }

    /// Records the frame the timeline was just applied for.
    ///
    /// # Errors
    ///
    /// Returns an invalid-specification error when a track cannot be read.
    pub fn record(&mut self, timeline: &Timeline) -> Result<(), Error> {
        for handle in &self.handles {
            let reading = timeline
                .reading(*handle)
                .ok_or_else(|| Error::InvalidSpec("timeline track cannot be read".to_owned()))?;
            self.sim_seconds.push(reading.time.seconds());
        }
        self.playback_seconds.push(timeline.time_seconds());
        Ok(())
    }

    /// Appends the frames of `next`, which must continue this sequence.
    ///
    /// A sequence rendered in consecutive pieces, for example one per resident
    /// trajectory interval, keeps a single manifest this way.
    ///
    /// # Errors
    ///
    /// Returns an invalid-specification error when `next` has another frame
    /// rate or exposure, does not start where this one ends, or describes
    /// different tracks.
    pub fn append(&mut self, next: &Self) -> Result<(), Error> {
        let same_tracks = self.units == next.units
            && self.dilations.len() == next.dilations.len()
            && self
                .dilations
                .iter()
                .zip(&next.dilations)
                .all(|(a, b)| a.to_bits() == b.to_bits());
        if self.schedule.fps.to_bits() != next.schedule.fps.to_bits()
            || self.schedule.quality != next.schedule.quality
            || self.schedule.range.end != next.schedule.range.start
            || !same_tracks
        {
            return Err(Error::InvalidSpec(
                "a manifest can only continue the sequence it follows".to_owned(),
            ));
        }
        self.schedule.range.end = next.schedule.range.end;
        self.playback_seconds
            .extend_from_slice(&next.playback_seconds);
        self.sim_seconds.extend_from_slice(&next.sim_seconds);
        Ok(())
    }

    /// Number of frames recorded.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.playback_seconds.len()
    }

    /// Whether no frame is recorded.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.playback_seconds.is_empty()
    }

    /// Playback seconds of every recorded frame, in order.
    #[must_use]
    pub fn playback_seconds(&self) -> &[f64] {
        &self.playback_seconds
    }

    /// Physical seconds of track `track` at recorded frame `frame`.
    #[must_use]
    pub fn sim_seconds(&self, frame: usize, track: usize) -> Option<f64> {
        if track >= self.handles.len() {
            return None;
        }
        self.sim_seconds
            .get(frame.checked_mul(self.handles.len())?.checked_add(track)?)
            .copied()
    }

    /// The manifest as a JSON value.
    #[must_use]
    pub fn to_json(&self) -> Value {
        let tracks =
            self.units
                .iter()
                .zip(&self.dilations)
                .enumerate()
                .map(|(track, (unit, dilation))| {
                    json!({
                        "track": track,
                        "unit": unit,
                        "dilation_sim_seconds_per_playback_second": dilation,
                    })
                });
        let width = self.handles.len();
        let numbered = (self.schedule.range.start..).zip(&self.playback_seconds);
        let frames = numbered.enumerate().map(|(row, (index, seconds))| {
            let start = row * width;
            let sim = self
                .sim_seconds
                .get(start..start + width)
                .into_iter()
                .flatten();
            let readings = sim
                .zip(&self.units)
                .map(|(time, unit)| json!({ "sim_seconds": time, "unit": unit }));
            json!({
                "frame": index,
                "playback_seconds": seconds,
                "tracks": readings.collect::<Vec<_>>(),
            })
        });
        json!({
            "caption": CAPTION,
            "schedule": {
                "fps": self.schedule.fps,
                "first_frame": self.schedule.range.start,
                "end_frame": self.schedule.range.end,
                "output": match self.schedule.quality {
                    MeasuredOutput::Converged => "converged",
                    MeasuredOutput::Interactive => "interactive",
                },
            },
            "tracks": tracks.collect::<Vec<_>>(),
            "frames": frames.collect::<Vec<_>>(),
        })
    }

    /// Writes the manifest as pretty JSON.
    ///
    /// # Errors
    ///
    /// Returns an I/O or serialization error.
    pub fn write_json(&self, path: impl AsRef<std::path::Path>) -> Result<(), Error> {
        let text = serde_json::to_string_pretty(&self.to_json())?;
        std::fs::write(path, text)?;
        Ok(())
    }
}
