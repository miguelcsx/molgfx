//! Bounded caller-supplied trajectory frames for GPU presentation.
//!
//! Parsing, decompression, periodic-boundary reconstruction, alignment and analysis remain
//! outside the renderer. A scene retains exactly the two decoded `f32` frames
//! needed for the current interpolation interval; a segment that selects
//! [`InterpolationKind::CubicHermite`] additionally retains the neighbouring frames
//! `before` and `after` (four in total) and only then. Times are physical [`SimTime`]
//! values held in `f64`, so a femtosecond step next to a millisecond timestamp keeps its
//! precision; only the interpolation fraction is narrowed at the GPU boundary.

#[cfg(test)]
#[path = "trajectory_tests.rs"]
mod tests;

use super::interpolation::{HermiteGaps, InterpolationKind};
use crate::CoreError;
use crate::time::{SimTime, narrow_to_f32};
use molgfx_math::{Aabb, Vec3};
use std::sync::Arc;

/// One topology-aligned decoded coordinate frame.
#[derive(Clone, PartialEq, Debug)]
pub struct TrajectoryFrame {
    index: u64,
    time: SimTime,
    positions: Arc<[[f32; 3]]>,
    provenance: Arc<str>,
}

impl TrajectoryFrame {
    /// Validates and retains one shared `f32` coordinate frame without a copy.
    ///
    /// `index` is the caller's stable trajectory-frame identity, used to avoid
    /// redundant uploads when only interpolation time changes. `time` accepts a
    /// [`SimTime`] or a bare number of seconds.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidTrajectory`] for empty/non-finite data,
    /// non-finite time or empty provenance.
    pub fn new(
        index: u64,
        time: impl Into<SimTime>,
        positions: Arc<[[f32; 3]]>,
        provenance: impl Into<Arc<str>>,
    ) -> Result<Self, CoreError> {
        let time = time.into();
        if !time.seconds().is_finite() {
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
            time,
            positions,
            provenance,
        })
    }

    /// Stable caller frame index.
    #[must_use]
    pub const fn index(&self) -> u64 {
        self.index
    }

    /// Physical trajectory time of this frame, in canonical seconds.
    #[must_use]
    pub const fn time_seconds(&self) -> f64 {
        self.time.seconds()
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

/// The active interpolation interval retained by one structure: two frames, plus
/// up to two neighbours when cubic Hermite interpolation is selected.
#[derive(Clone, PartialEq, Debug)]
pub struct TrajectorySegment {
    start: TrajectoryFrame,
    end: TrajectoryFrame,
    kind: InterpolationKind,
    before: Option<TrajectoryFrame>,
    after: Option<TrajectoryFrame>,
    sample_seconds: f64,
    interpolation: f64,
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
        sample: impl Into<SimTime>,
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
        if !start.time.is_before(end.time) {
            return Err(CoreError::InvalidTrajectory {
                reason: "trajectory frame times must increase",
            });
        }
        let mut segment = Self {
            start,
            end,
            kind: InterpolationKind::Linear,
            before: None,
            after: None,
            sample_seconds: 0.0,
            interpolation: 0.0,
        };
        segment.set_sample_time(sample)?;
        Ok(segment)
    }

    /// Selects cubic Hermite interpolation and retains the neighbouring frames.
    ///
    /// `before` precedes the segment start and `after` follows its end; either may
    /// be absent at the ends of a trajectory, in which case that tangent falls
    /// back to the one-sided difference (both absent is exactly linear). Tangents
    /// are finite differences over the `f64` frame times, see
    /// [`InterpolationKind::CubicHermite`].
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidTrajectory`] for a neighbour with a different
    /// atom count or one that is not strictly before/after the segment in both
    /// frame index and time.
    pub fn with_cubic_hermite(
        mut self,
        before: Option<TrajectoryFrame>,
        after: Option<TrajectoryFrame>,
    ) -> Result<Self, CoreError> {
        let count = self.start.positions.len();
        let ordered = |earlier: &TrajectoryFrame, later: &TrajectoryFrame| {
            earlier.index < later.index && earlier.time.is_before(later.time)
        };
        let valid = before
            .iter()
            .all(|frame| frame.positions.len() == count && ordered(frame, &self.start))
            && after
                .iter()
                .all(|frame| frame.positions.len() == count && ordered(&self.end, frame));
        if !valid {
            return Err(CoreError::InvalidTrajectory {
                reason: "neighbour frames must match the atom count and bracket the segment",
            });
        }
        self.kind = InterpolationKind::CubicHermite;
        self.before = before;
        self.after = after;
        Ok(self)
    }

    /// Updates presentation time without replacing either resident frame.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidTrajectory`] outside the closed interval.
    pub fn set_sample_time(&mut self, sample: impl Into<SimTime>) -> Result<(), CoreError> {
        let sample_seconds = sample.into().seconds();
        let start_seconds = self.start.time_seconds();
        let end_seconds = self.end.time_seconds();
        if !sample_seconds.is_finite()
            || sample_seconds < start_seconds
            || sample_seconds > end_seconds
        {
            return Err(CoreError::InvalidTrajectory {
                reason: "sample time must lie within the resident frame interval",
            });
        }
        self.sample_seconds = sample_seconds;
        self.interpolation = (sample_seconds - start_seconds) / (end_seconds - start_seconds);
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

    /// Frame preceding [`Self::start`], retained only for cubic Hermite.
    #[must_use]
    pub const fn before(&self) -> Option<&TrajectoryFrame> {
        self.before.as_ref()
    }

    /// Frame following [`Self::end`], retained only for cubic Hermite.
    #[must_use]
    pub const fn after(&self) -> Option<&TrajectoryFrame> {
        self.after.as_ref()
    }

    /// How the resident frames are blended.
    #[must_use]
    pub const fn interpolation_kind(&self) -> InterpolationKind {
        self.kind
    }

    /// Number of decoded frames held by this segment: two, plus each retained neighbour.
    #[must_use]
    pub const fn resident_frames(&self) -> usize {
        2 + self.before.is_some() as usize + self.after.is_some() as usize
    }

    fn hermite_gaps(&self) -> HermiteGaps {
        HermiteGaps {
            interval: self.end.time_seconds() - self.start.time_seconds(),
            before: self
                .before
                .as_ref()
                .map(|frame| self.start.time_seconds() - frame.time_seconds()),
            after: self
                .after
                .as_ref()
                .map(|frame| frame.time_seconds() - self.end.time_seconds()),
        }
    }

    /// Blend weights over `[before, start, end, after]` at interpolation fraction `alpha`.
    ///
    /// Linear gives `[0, 1 - alpha, alpha, 0]`. Cubic Hermite folds the tangents
    /// into the weights, so a position is the weighted sum of the four frames.
    /// The weights sum to one. Computed in `f64`; the caller narrows for the GPU.
    #[must_use]
    pub fn blend_weights(&self, alpha: f64) -> [f64; 4] {
        match self.kind {
            InterpolationKind::Linear => [0.0, 1.0 - alpha, alpha, 0.0],
            InterpolationKind::CubicHermite => self.hermite_gaps().weights(alpha),
        }
    }

    /// Current sample time, in canonical seconds.
    #[must_use]
    pub const fn sample_seconds(&self) -> f64 {
        self.sample_seconds
    }

    /// Exact linear interpolation fraction in `[0, 1]`.
    #[must_use]
    pub const fn interpolation(&self) -> f64 {
        self.interpolation
    }

    /// Atom count shared by both frames.
    #[must_use]
    pub fn atom_count(&self) -> usize {
        self.start.positions.len()
    }

    /// Conservative bound containing both frames and every interpolated sample.
    ///
    /// A Hermite segment can overshoot its frames, so its bound also contains the
    /// Bezier control points `p0 + T0 / 3` and `p1 - T1 / 3` of every atom, whose
    /// convex hull encloses the cubic.
    #[must_use]
    pub fn union_aabb(&self) -> Aabb {
        let frames = self.start.aabb().union(&self.end.aabb());
        if self.kind == InterpolationKind::Linear {
            return frames;
        }
        let controls = self.hermite_gaps().control_weights();
        let neighbour = |frame: &Option<TrajectoryFrame>, fallback: &'_ TrajectoryFrame| match frame
        {
            Some(frame) => frame.positions.clone(),
            None => fallback.positions.clone(),
        };
        let ordered = [
            neighbour(&self.before, &self.start),
            self.start.positions.clone(),
            self.end.positions.clone(),
            neighbour(&self.after, &self.end),
        ];
        let points = (0..self.start.positions.len()).flat_map(|atom| {
            controls.map(|weights| {
                let axis = |axis: usize| {
                    let sum: f64 = ordered
                        .iter()
                        .zip(weights)
                        .map(|(frame, weight)| weight * f64::from(frame[atom][axis]))
                        .sum();
                    narrow_to_f32(sum)
                };
                Some(Vec3::new(axis(0)?, axis(1)?, axis(2)?))
            })
        });
        frames.union(&Aabb::from_points(points.flatten()))
    }
}
