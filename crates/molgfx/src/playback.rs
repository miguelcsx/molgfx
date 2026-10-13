//! Stepping a long trajectory through a scene's resident frame pair.

use crate::TrajectoryFrame;
use std::collections::VecDeque;
use std::fmt;

/// Frames kept converted so a seek inside one segment converts none.
const CACHE_FRAMES: usize = 4;

/// Why a set of frame times cannot drive playback.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimelineError {
    /// Fewer than two frames, or a different number of times than frames.
    FrameCount,
    /// Times are not strictly ascending.
    NotAscending,
}

impl fmt::Display for TimelineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::FrameCount => "playback needs at least two frames and one time per frame",
            Self::NotAscending => "times must be strictly ascending",
        })
    }
}

impl std::error::Error for TimelineError {}

/// Where a time falls: the segment `[index, index + 1]` and the clamped time.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Position {
    /// Index of the segment's first frame.
    pub segment: usize,
    /// The requested time clamped to the first and last frame time.
    pub time: f32,
}

/// Strictly ascending frame times.
#[derive(Clone, Debug, PartialEq)]
pub struct Timeline {
    times: Vec<f32>,
}

impl Timeline {
    /// Validates one time per frame.
    ///
    /// # Errors
    ///
    /// Returns [`TimelineError`] for fewer than two frames, a count mismatch,
    /// or times that do not strictly ascend.
    pub fn new(frames: usize, times: Vec<f32>) -> Result<Self, TimelineError> {
        if frames < 2 || frames != times.len() {
            return Err(TimelineError::FrameCount);
        }
        if times.windows(2).any(|pair| pair[1] <= pair[0]) {
            return Err(TimelineError::NotAscending);
        }
        Ok(Self { times })
    }

    /// The frame times.
    #[must_use]
    pub fn times(&self) -> &[f32] {
        &self.times
    }

    /// The time of frame `index`, or `None` past the end.
    #[must_use]
    pub fn time(&self, index: usize) -> Option<f32> {
        self.times.get(index).copied()
    }

    /// The segment containing `time`, clamped to the first and last frame.
    #[must_use]
    pub fn locate(&self, time: f32) -> Position {
        let last = self.times.len() - 1;
        let time = time.max(self.times[0]).min(self.times[last]);
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
