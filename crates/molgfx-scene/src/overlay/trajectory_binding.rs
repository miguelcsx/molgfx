//! Shared resident coordinate intervals and their validated presentation time.

use crate::{DataSource, Error};
use std::sync::{Arc, OnceLock};

#[cfg(test)]
#[path = "trajectory_binding_tests.rs"]
mod tests;

/// One topology-aligned coordinate frame supplied outside [`crate::SceneSpec`].
///
/// A frame is bulk data, so it travels as a runtime binding rather than in the
/// specification: the descriptor declares how many frames a source holds, and
/// the binding hands over the two the renderer samples between.
#[derive(Clone, Debug)]
pub struct TrajectoryFrame {
    index: u64,
    time_seconds: f64,
    positions: Arc<[[f32; 3]]>,
    native: Arc<OnceLock<molgfx_core::TrajectoryFrame>>,
}

impl PartialEq for TrajectoryFrame {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index
            && self.time_seconds.eq(&other.time_seconds)
            && self.positions == other.positions
    }
}

impl TrajectoryFrame {
    /// Declares one frame without copying its coordinates.
    ///
    /// `index` is the frame's stable identity in its source, which is what lets
    /// the renderer skip an upload when only the sample time moved.
    #[must_use]
    pub fn new(index: u64, time_seconds: f64, positions: Arc<[[f32; 3]]>) -> Self {
        Self {
            index,
            time_seconds,
            positions,
            native: Arc::new(OnceLock::new()),
        }
    }

    /// This frame's stable identity in its source.
    #[must_use]
    pub const fn index(&self) -> u64 {
        self.index
    }

    /// Presentation time this frame was recorded at, in seconds.
    #[must_use]
    pub const fn time_seconds(&self) -> f64 {
        self.time_seconds
    }

    /// The frame's coordinates, one per atom.
    #[must_use]
    pub fn positions(&self) -> &[[f32; 3]] {
        &self.positions
    }

    /// Shares the original immutable coordinate owner for host transport in O(1).
    #[must_use]
    pub fn shared_positions(&self) -> Arc<[[f32; 3]]> {
        self.positions.clone()
    }

    pub(crate) fn same_frame(&self, other: &Self) -> bool {
        self.index == other.index
            && self.time_seconds.to_bits() == other.time_seconds.to_bits()
            && Arc::ptr_eq(&self.positions, &other.positions)
    }

    /// Validates a new column once in O(atoms); later captures and interval swaps are O(1).
    pub(crate) fn native(&self) -> Result<molgfx_core::TrajectoryFrame, Error> {
        if let Some(frame) = self.native.get() {
            return Ok(frame.clone());
        }
        let frame = molgfx_core::TrajectoryFrame::new(
            self.index,
            self.time_seconds,
            Arc::clone(&self.positions),
            "molgfx:trajectory",
        )?;
        let _ = self.native.set(frame.clone());
        Ok(frame)
    }
}

/// Two resident frames a trajectory descriptor samples between.
///
/// The renderer holds exactly one interpolation interval per structure, so the
/// bound data is the pair currently presented plus the sample time read from it.
/// A different pair replaces the resident one; advancing within the interval is
/// [`Scene::set_trajectory_time`](crate::Scene::set_trajectory_time).
#[derive(Clone, Debug)]
pub struct TrajectoryBinding {
    source: DataSource,
    start: TrajectoryFrame,
    end: TrajectoryFrame,
    sample_seconds: f64,
}

impl TrajectoryBinding {
    pub(crate) fn same_input(&self, other: &Self) -> bool {
        self.start.same_frame(&other.start)
            && self.end.same_frame(&other.end)
            && self.sample_seconds.to_bits() == other.sample_seconds.to_bits()
    }

    /// Portable source identity of this resident interval.
    #[must_use]
    pub const fn source(&self) -> &DataSource {
        &self.source
    }

    /// Borrows both retained coordinate frames in presentation order.
    #[must_use]
    pub const fn frames(&self) -> (&TrajectoryFrame, &TrajectoryFrame) {
        (&self.start, &self.end)
    }

    /// Current presentation time, in seconds.
    #[must_use]
    pub const fn sample_time(&self) -> f64 {
        self.sample_seconds
    }

    /// Captures this exact interval as a scientific calculation input.
    ///
    /// # Errors
    /// Rejects malformed frames or a different molecular atom count.
    pub fn capture_pose(
        &self,
        source: &crate::source::MolecularSource,
    ) -> Result<crate::source::MolecularPose, Error> {
        Ok(crate::source::MolecularPose::new(
            source.clone(),
            Some(self.native(source.coordinates().len())?),
        )?)
    }

    /// Declares the ordered pair the renderer should hold.
    #[must_use]
    pub fn new(source: DataSource, start: TrajectoryFrame, end: TrajectoryFrame) -> Self {
        Self {
            source,
            start,
            end,
            sample_seconds: 0.0,
        }
    }

    /// Sets the presentation time the pair is sampled at.
    #[must_use]
    pub fn sample_seconds(mut self, sample_seconds: f64) -> Self {
        self.sample_seconds = sample_seconds;
        self
    }

    pub(crate) fn content_hash(&self) -> &str {
        &self.source.content_hash
    }

    pub(super) fn retain_sample_time(&mut self, segment: &molgfx_core::TrajectorySegment) {
        if self.start.index == segment.start().index()
            && self.end.index == segment.end().index()
            && std::ptr::eq(self.start.positions.as_ref(), segment.start().positions())
            && std::ptr::eq(self.end.positions.as_ref(), segment.end().positions())
        {
            self.sample_seconds = segment.sample_seconds();
        }
    }

    /// Confirms the pair agrees with the portable descriptor it satisfies.
    pub(crate) fn matches(&self, spec: &crate::TrajectorySpec) -> Result<(), Error> {
        // The descriptor declares a frame count; the binding supplies a pair of
        // it. A pair reaching outside the declared range would present a frame
        // the specification does not claim exists.
        let highest = self.start.index.max(self.end.index);
        if self.start.index >= self.end.index
            || highest >= spec.frame_count
            || !self.sample_seconds.is_finite()
        {
            return Err(Error::InvalidSpec(format!(
                "trajectory source '{}' does not match its descriptor",
                self.content_hash()
            )));
        }
        Ok(())
    }

    pub(crate) fn validate(&self) -> Result<(), Error> {
        if self.content_hash().trim().is_empty()
            || self.start.positions.is_empty()
            || self.start.positions.len() != self.end.positions.len()
            || self.start.native().is_err()
            || self.end.native().is_err()
            || !self.start.time_seconds.is_finite()
            || !self.end.time_seconds.is_finite()
            || self.start.time_seconds >= self.end.time_seconds
            || self.start.index >= self.end.index
            || !self.sample_seconds.is_finite()
        {
            return Err(Error::InvalidSpec(
                "trajectory binding requires a content hash, two equal-length finite frames with \
                 increasing indices and times, and a finite sample time"
                    .to_owned(),
            ));
        }
        Ok(())
    }

    /// The resident interval, validated against one structure's atom count.
    ///
    /// # Errors
    ///
    /// Returns an invalid-specification error when the frames do not match the
    /// structure's topology.
    pub(crate) fn native(
        &self,
        atom_count: usize,
    ) -> Result<molgfx_core::TrajectorySegment, Error> {
        if self.start.positions.len() != atom_count {
            return Err(Error::InvalidSpec(format!(
                "trajectory source '{}' holds {} atoms but its structure has {atom_count}",
                self.content_hash(),
                self.start.positions.len()
            )));
        }
        Ok(molgfx_core::TrajectorySegment::new(
            self.start.native()?,
            self.end.native()?,
            self.sample_seconds,
        )?)
    }
}
