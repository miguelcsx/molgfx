//! Bounded caller-supplied trajectory frames for GPU presentation.
//!
//! Parsing, decompression, periodic-boundary reconstruction, alignment and analysis remain
//! outside the renderer. A scene retains exactly the two decoded `f32` frames
//! needed for the current interpolation interval.

#[cfg(test)]
#[path = "trajectory_tests.rs"]
mod tests;

use crate::CoreError;
use pdviewx_math::{Aabb, Vec3};
use std::sync::Arc;

/// One topology-aligned decoded coordinate frame.
#[derive(Clone, PartialEq, Debug)]
pub struct TrajectoryFrame {
    index: u64,
    time_seconds: f32,
    positions: Arc<[[f32; 3]]>,
    provenance: Arc<str>,
}

impl TrajectoryFrame {
    /// Validates and retains one shared `f32` coordinate frame without a copy.
    ///
    /// `index` is the caller's stable trajectory-frame identity, used to avoid
    /// redundant uploads when only interpolation time changes.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidTrajectory`] for empty/non-finite data,
    /// non-finite time or empty provenance.
    pub fn new(
        index: u64,
        time_seconds: f32,
        positions: Arc<[[f32; 3]]>,
        provenance: impl Into<Arc<str>>,
    ) -> Result<Self, CoreError> {
        if !time_seconds.is_finite() {
            return Err(CoreError::InvalidTrajectory {
                reason: "frame time must be finite",
            });
        }
        if positions.is_empty() || positions.iter().flatten().any(|value| !value.is_finite()) {
            return Err(CoreError::InvalidTrajectory {
                reason: "frame positions must be non-empty and finite",
            });
        }
        let provenance = provenance.into();
        if provenance.trim().is_empty() {
            return Err(CoreError::InvalidTrajectory {
                reason: "frame provenance must not be empty",
            });
        }
        Ok(Self {
            index,
            time_seconds,
            positions,
            provenance,
        })
    }

    /// Stable caller frame index.
    #[must_use]
    pub const fn index(&self) -> u64 {
        self.index
    }

    /// Physical or logical trajectory time in seconds.
    #[must_use]
    pub const fn time_seconds(&self) -> f32 {
        self.time_seconds
    }

    /// Topology-aligned `f32` positions shared with the caller.
    #[must_use]
    pub fn positions(&self) -> &[[f32; 3]] {
        &self.positions
    }

    /// Source decoder, dataset or simulation identifier.
    #[must_use]
    pub fn provenance(&self) -> &str {
        &self.provenance
    }

    /// Tight source-frame bound.
    #[must_use]
    pub fn aabb(&self) -> Aabb {
        Aabb::from_points(self.positions.iter().copied().map(Vec3::from_array))
    }
}

/// The active two-frame interpolation interval retained by one structure.
#[derive(Clone, PartialEq, Debug)]
pub struct TrajectorySegment {
    start: TrajectoryFrame,
    end: TrajectoryFrame,
    sample_seconds: f32,
    interpolation: f32,
}

impl TrajectorySegment {
    /// Creates an ordered two-frame segment sampled within its closed interval.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidTrajectory`] for different topology lengths,
    /// unordered timestamps/indices or a sample outside the interval.
    pub fn new(
        start: TrajectoryFrame,
        end: TrajectoryFrame,
        sample_seconds: f32,
    ) -> Result<Self, CoreError> {
        if start.positions.len() != end.positions.len() {
            return Err(CoreError::InvalidTrajectory {
                reason: "trajectory frames must have equal atom counts",
            });
        }
        if start.index >= end.index {
            return Err(CoreError::InvalidTrajectory {
                reason: "trajectory frame indices must increase",
            });
        }
        if start.time_seconds >= end.time_seconds {
            return Err(CoreError::InvalidTrajectory {
                reason: "trajectory frame times must increase",
            });
        }
        let mut segment = Self {
            start,
            end,
            sample_seconds: 0.0,
            interpolation: 0.0,
        };
        segment.set_sample_time(sample_seconds)?;
        Ok(segment)
    }

    /// Updates presentation time without replacing either resident frame.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidTrajectory`] outside the closed interval.
    pub fn set_sample_time(&mut self, sample_seconds: f32) -> Result<(), CoreError> {
        if !sample_seconds.is_finite()
            || sample_seconds < self.start.time_seconds
            || sample_seconds > self.end.time_seconds
        {
            return Err(CoreError::InvalidTrajectory {
                reason: "sample time must lie within the resident frame interval",
            });
        }
        let duration = self.end.time_seconds - self.start.time_seconds;
        self.sample_seconds = sample_seconds;
        self.interpolation = (sample_seconds - self.start.time_seconds) / duration;
        Ok(())
    }

    /// Earlier resident frame.
    #[must_use]
    pub const fn start(&self) -> &TrajectoryFrame {
        &self.start
    }

    /// Later resident frame.
    #[must_use]
    pub const fn end(&self) -> &TrajectoryFrame {
        &self.end
    }

    /// Current sample time.
    #[must_use]
    pub const fn sample_seconds(&self) -> f32 {
        self.sample_seconds
    }

    /// Exact linear interpolation fraction in `[0, 1]`.
    #[must_use]
    pub const fn interpolation(&self) -> f32 {
        self.interpolation
    }

    /// Atom count shared by both frames.
    #[must_use]
    pub fn atom_count(&self) -> usize {
        self.start.positions.len()
    }

    /// Conservative bound containing both frames and every linear interpolation.
    #[must_use]
    pub fn union_aabb(&self) -> Aabb {
        self.start.aabb().union(&self.end.aabb())
    }
}
