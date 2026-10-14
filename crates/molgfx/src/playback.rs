//! Stepping a long trajectory through a scene's resident frame pair.

use crate::TrajectoryFrame;
use crate::time::SimTime;
use std::collections::VecDeque;
use std::fmt;

/// Frames kept converted so a seek inside one segment converts none.
const CACHE_FRAMES: usize = 4;

/// Why a set of frame times cannot drive playback.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameTableError {
    /// Fewer than two frames, or a different number of times than frames.
    FrameCount,
    /// A frame time is NaN or infinite, as molframe reports unrecorded frames.
    NonFinite,
    /// Times are not strictly ascending.
    NotAscending,
}

impl fmt::Display for FrameTableError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::FrameCount => "playback needs at least two frames and one time per frame",
            Self::NonFinite => "frame times must be finite seconds",
            Self::NotAscending => "times must be strictly ascending",
        })
    }
}

impl std::error::Error for FrameTableError {}

/// Where a time falls: the segment `[index, index + 1]` and the clamped time.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Position {
    /// Index of the segment's first frame.
    pub segment: usize,
    /// The requested time clamped to the first and last frame time, in seconds.
    pub time: SimTime,
}

/// Strictly ascending, finite frame times in seconds.
#[derive(Clone, Debug, PartialEq)]
pub struct FrameTable {
    times: Vec<SimTime>,
}

impl FrameTable {
    /// Validates one time per frame.
    ///
    /// The finiteness check runs first, so the ordering comparison never sees
    /// a NaN; a NaN compares false in both directions and would otherwise pass.
    ///
    /// # Errors
    ///
    /// Returns [`FrameTableError`] for fewer than two frames, a count mismatch,
    /// a non-finite time, or times that do not strictly ascend.
    pub fn new(frames: usize, times: Vec<SimTime>) -> Result<Self, FrameTableError> {
        if frames < 2 || frames != times.len() {
            return Err(FrameTableError::FrameCount);
        }
        if times.iter().any(|time| !time.seconds().is_finite()) {
            return Err(FrameTableError::NonFinite);
        }
        if times.windows(2).any(|pair| !pair[0].is_before(pair[1])) {
            return Err(FrameTableError::NotAscending);
        }
        Ok(Self { times })
    }

    /// The frame times in seconds.
    #[must_use]
    pub fn times(&self) -> &[SimTime] {
        &self.times
    }

    /// The time of frame `index` in seconds, or `None` past the end.
    #[must_use]
    pub fn time(&self, index: usize) -> Option<SimTime> {
        self.times.get(index).copied()
    }

    /// The segment containing `time`, clamped to the first and last frame.
    #[must_use]
    pub fn locate(&self, time: SimTime) -> Position {
        let last = self.times.len() - 1;
        let seconds = time
            .seconds()
            .max(self.times[0].seconds())
            .min(self.times[last].seconds());
        let time = SimTime::from(seconds);
        let after = self.times.partition_point(|stamp| *stamp <= time);
        Position {
            segment: after.saturating_sub(1).min(last - 1),
            time,
        }
    }
}

/// The few most recently converted frames, keyed by frame index.
#[derive(Debug, Default)]
pub struct FrameCache {
    frames: VecDeque<(usize, TrajectoryFrame)>,
}

impl FrameCache {
    /// Returns frame `index`, converting it with `convert` at most once while
    /// it stays among the most recent few.
    ///
    /// # Errors
    ///
    /// Propagates the error of `convert`.
    pub fn get_or_convert<E>(
        &mut self,
        index: usize,
        convert: impl FnOnce() -> Result<TrajectoryFrame, E>,
    ) -> Result<TrajectoryFrame, E> {
        let cached = self.frames.iter().position(|(key, _)| *key == index);
        if let Some(entry) = cached.and_then(|at| self.frames.remove(at)) {
            let frame = entry.1.clone();
            self.frames.push_back(entry);
            return Ok(frame);
        }
        let frame = convert()?;
        self.frames.push_back((index, frame.clone()));
        while self.frames.len() > CACHE_FRAMES {
            self.frames.pop_front();
        }
        Ok(frame)
    }
}
