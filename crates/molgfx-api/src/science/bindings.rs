//! Runtime bulk data for scientific descriptors, and renderer-handle inspection.

use crate::{DataSource, Error, VolumeSpec};
use molgfx_math::Vec3;
use std::collections::BTreeMap;
use std::sync::Arc;

/// Immutable shared scalar grid supplied outside [`crate::SceneSpec`].
///
/// A density descriptor stays portable; its voxels are a runtime binding, so
/// the same specification resolves on every host that holds the data.
#[derive(Clone, Debug)]
pub struct VolumeBinding {
    source: DataSource,
    dimensions: [u32; 3],
    spacing: [f32; 3],
    origin: [f32; 3],
    values: Arc<[f32]>,
}

impl VolumeBinding {
    /// Declares one shared density grid without copying its values.
    #[must_use]
    pub fn new(source: DataSource, dimensions: [u32; 3], values: Arc<[f32]>) -> Self {
        Self {
            source,
            dimensions,
            spacing: [1.0; 3],
            origin: [0.0; 3],
            values,
        }
    }

    /// Sets positive voxel spacing in ångström.
    #[must_use]
    pub fn spacing(mut self, spacing: [f32; 3]) -> Self {
        self.spacing = spacing;
        self
    }

    /// Sets the world-space grid origin in ångström.
    #[must_use]
    pub fn origin(mut self, origin: [f32; 3]) -> Self {
        self.origin = origin;
        self
    }

    pub(crate) fn content_hash(&self) -> &str {
        &self.source.content_hash
    }

    /// Confirms the grid agrees with the portable descriptor it satisfies.
    pub(crate) fn matches(&self, spec: &VolumeSpec) -> Result<(), Error> {
        // Bit-exact agreement, not approximate: the descriptor promises these
        // numbers and the grid is the data that must satisfy them.
        if self.dimensions != spec.dimensions
            || !same_bits(self.spacing, spec.spacing)
            || !same_bits(self.origin, spec.origin)
        {
            return Err(Error::InvalidSpec(format!(
                "density source '{}' does not match its descriptor",
                self.content_hash()
            )));
        }
        Ok(())
    }

    pub(crate) fn validate(&self) -> Result<(), Error> {
        let voxels = self
            .dimensions
            .iter()
            .try_fold(1u64, |product, &dimension| {
                product.checked_mul(u64::from(dimension))
            })
            .and_then(|voxels| usize::try_from(voxels).ok());
        let spacing_is_valid = self
            .spacing
            .iter()
            .all(|value| value.is_finite() && *value > 0.0);
        if self.content_hash().trim().is_empty()
            || self.dimensions.iter().any(|dimension| *dimension < 2)
            || !spacing_is_valid
            || !self.origin.iter().all(|value| value.is_finite())
            || voxels != Some(self.values.len())
            || self.values.iter().any(|value| !value.is_finite())
        {
            return Err(Error::InvalidSpec(
                "volume binding requires a content hash, a grid of at least two voxels per axis, positive finite spacing, a finite origin, and one finite value per voxel"
                    .to_owned(),
            ));
        }
        Ok(())
    }

    pub(crate) fn native(&self) -> Result<molgfx_core::ScalarVolume, Error> {
        Ok(molgfx_core::ScalarVolume::from_spacing(
            self.dimensions,
            Vec3::from_array(self.origin),
            Vec3::from_array(self.spacing),
            Arc::clone(&self.values),
        )?)
    }
}

/// One topology-aligned coordinate frame supplied outside [`crate::SceneSpec`].
///
/// A frame is bulk data, so it travels as a runtime binding rather than in the
/// specification: the descriptor declares how many frames a source holds, and
/// the binding hands over the two the renderer samples between.
#[derive(Clone, PartialEq, Debug)]
pub struct TrajectoryFrame {
    index: u64,
    time_seconds: f32,
    positions: Arc<[[f32; 3]]>,
}

impl TrajectoryFrame {
    /// Declares one frame without copying its coordinates.
    ///
    /// `index` is the frame's stable identity in its source, which is what lets
    /// the renderer skip an upload when only the sample time moved.
    #[must_use]
    pub fn new(index: u64, time_seconds: f32, positions: Arc<[[f32; 3]]>) -> Self {
        Self {
            index,
            time_seconds,
            positions,
        }
    }

    /// This frame's stable identity in its source.
    #[must_use]
    pub const fn index(&self) -> u64 {
        self.index
    }

    /// Presentation time this frame was recorded at, in seconds.
    #[must_use]
    pub const fn time_seconds(&self) -> f32 {
        self.time_seconds
    }

    /// The frame's coordinates, one per atom.
    #[must_use]
    pub fn positions(&self) -> &[[f32; 3]] {
        &self.positions
    }

    /// Retains this frame as core's own frame type.
    fn native(&self) -> Result<molgfx_core::TrajectoryFrame, Error> {
        Ok(molgfx_core::TrajectoryFrame::new(
            self.index,
            self.time_seconds,
            Arc::clone(&self.positions),
            "molgfx:trajectory",
        )?)
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
    sample_seconds: f32,
}

impl TrajectoryBinding {
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
    pub fn sample_seconds(mut self, sample_seconds: f32) -> Self {
        self.sample_seconds = sample_seconds;
        self
    }

    pub(crate) fn content_hash(&self) -> &str {
        &self.source.content_hash
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
            || self
                .start
                .positions
                .iter()
                .chain(self.end.positions.iter())
                .flatten()
                .any(|value| !value.is_finite())
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

/// Runtime bulk data a resolved scene's scientific descriptors can reach.
#[derive(Clone, Debug, Default)]
pub(crate) struct ScienceBindings {
    volumes: BTreeMap<Box<str>, VolumeBinding>,
    trajectories: BTreeMap<Box<str>, TrajectoryBinding>,
}

impl ScienceBindings {
    /// Registers one density grid under its portable content hash.
    ///
    /// # Errors
    ///
    /// Returns an invalid-specification error for a malformed grid or a content
    /// hash that is already bound.
    pub(crate) fn insert(&mut self, binding: VolumeBinding) -> Result<(), Error> {
        binding.validate()?;
        let key = binding.content_hash().to_owned().into_boxed_str();
        if self.volumes.contains_key(&key) {
            return Err(Error::InvalidSpec(format!(
                "density source '{key}' is already bound"
            )));
        }
        let _previous = self.volumes.insert(key, binding);
        Ok(())
    }

    /// Resolves the grid bound to one portable content hash.
    pub(crate) fn volume(&self, content_hash: &str) -> Option<&VolumeBinding> {
        self.volumes.get(content_hash)
    }

    /// Registers one frame pair under its portable content hash.
    ///
    /// # Errors
    ///
    /// Returns an invalid-specification error for a malformed pair or a content
    /// hash that is already bound.
    pub(crate) fn insert_trajectory(&mut self, binding: TrajectoryBinding) -> Result<(), Error> {
        binding.validate()?;
        let key = binding.content_hash().to_owned().into_boxed_str();
        if self.trajectories.contains_key(&key) {
            return Err(Error::InvalidSpec(format!(
                "trajectory source '{key}' is already bound"
            )));
        }
        let _previous = self.trajectories.insert(key, binding);
        Ok(())
    }

    /// Resolves the frame pair bound to one portable content hash.
    pub(crate) fn trajectory(&self, content_hash: &str) -> Option<&TrajectoryBinding> {
        self.trajectories.get(content_hash)
    }
}

/// Renderer-side handle counts for each exposed scientific capability.
///
/// A portable descriptor that never reached the renderer counts zero: a density
/// volume with no runtime binding, for example, is deliberately unresolved.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct ScientificHandles {
    /// Density grids resolved against a runtime binding.
    pub volumes: usize,
    /// Labels stored as annotations.
    pub labels: usize,
    /// Geometry-backed measurements.
    pub measurements: usize,
    /// Explicit interaction edges.
    pub interactions: usize,
    /// Structures holding a resident trajectory frame pair.
    pub trajectories: usize,
}

/// Whether two coordinate triples are bit-identical.
///
/// Descriptor agreement is exact by contract; `-0.0` and `0.0` are treated as
/// different so a signed origin cannot pass unnoticed.
fn same_bits(left: [f32; 3], right: [f32; 3]) -> bool {
    left.map(f32::to_bits) == right.map(f32::to_bits)
}
