//! Runtime bulk data for overlay descriptors, and renderer-handle inspection.

use super::TrajectoryBinding;
use crate::{DataSource, Error, VolumeSpec};
use molgfx_math::Mat4;
use num_traits::ToPrimitive as _;
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
    voxel_to_world: [f32; 16],
    values: Arc<[f32]>,
}

impl VolumeBinding {
    /// Portable content identity of this immutable scalar column.
    #[must_use]
    pub const fn source(&self) -> &DataSource {
        &self.source
    }
    /// Borrows the immutable scalar column for bulk transport or persistence.
    #[must_use]
    pub fn values(&self) -> &[f32] {
        &self.values
    }
    /// Declares one shared density grid without copying its values.
    #[must_use]
    pub fn new(source: DataSource, dimensions: [u32; 3], values: Arc<[f32]>) -> Self {
        Self {
            source,
            dimensions,
            voxel_to_world: Mat4::IDENTITY.to_cols_array(),
            values,
        }
    }

    /// Sets the column-major voxel-to-world mapping without losing skew.
    #[must_use]
    pub fn affine(mut self, affine: [f32; 16]) -> Self {
        self.voxel_to_world = affine;
        self
    }

    /// Builds a voxel basis from an origin, axis lengths and cell angles.
    ///
    /// # Errors
    /// Returns an error for non-finite or degenerate cell geometry.
    pub fn from_cell(
        self,
        origin: [f32; 3],
        spacing: [f32; 3],
        cell_angles_deg: [f32; 3],
    ) -> Result<Self, Error> {
        Ok(self.affine(super::volume_spec::cell_affine(
            origin,
            spacing,
            cell_angles_deg,
        )?))
    }

    pub(crate) fn content_hash(&self) -> &str {
        &self.source.content_hash
    }

    /// Confirms the grid agrees with the portable descriptor it satisfies.
    pub(crate) fn matches(&self, spec: &VolumeSpec) -> Result<(), Error> {
        // Bit-exact agreement, not approximate: the descriptor promises these
        // numbers and the grid is the data that must satisfy them.
        if self.dimensions != spec.dimensions
            || !same_bits(self.voxel_to_world, spec.voxel_to_world)
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
        super::volume_spec::validate_affine(self.voxel_to_world)?;
        if self.content_hash().trim().is_empty()
            || self.dimensions.iter().any(|dimension| *dimension < 2)
            || voxels != Some(self.values.len())
            || self.values.iter().any(|value| !value.is_finite())
        {
            return Err(Error::InvalidSpec(
                "volume binding requires a content hash, a grid of at least two voxels per axis, a finite affine matrix, and one finite value per voxel"
                    .to_owned(),
            ));
        }
        Ok(())
    }

    pub(crate) fn native(&self) -> Result<molgfx_core::ScalarVolume, Error> {
        Ok(molgfx_core::ScalarVolume::new(
            self.dimensions,
            Mat4::from_cols_array(&self.voxel_to_world),
            Arc::clone(&self.values),
        )?)
    }
}

/// Statistics recorded for a density map, used to convert sigma controls into values.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct VolumeStatistics {
    /// Mean value of the density map.
    pub mean: f32,
    /// Population standard deviation of the density map.
    pub sigma: f32,
}

impl VolumeBinding {
    /// Computes finite mean and population standard deviation once at acquisition time.
    ///
    /// # Errors
    ///
    /// Returns an error when the volume has no voxels or its statistics are not
    /// finite.
    pub fn statistics(&self) -> Result<VolumeStatistics, Error> {
        if self.values.is_empty() {
            return Err(Error::InvalidSpec("density map has no voxels".to_owned()));
        }
        let count = u32::try_from(self.values.len()).map_err(|_| {
            Error::InvalidSpec("density map has too many voxels for statistics".to_owned())
        })?;
        let count = count.to_f32().ok_or_else(|| {
            Error::InvalidSpec("density map voxel count is too large for statistics".to_owned())
        })?;
        let mean = self.values.iter().copied().sum::<f32>() / count;
        let variance = self
            .values
            .iter()
            .map(|value| {
                let delta = *value - mean;
                delta * delta
            })
            .sum::<f32>()
            / count;
        let sigma = variance.sqrt();
        if !mean.is_finite() || !sigma.is_finite() {
            return Err(Error::InvalidSpec(
                "density map statistics are not finite".to_owned(),
            ));
        }
        Ok(VolumeStatistics { mean, sigma })
    }

    /// Converts a sigma multiplier to an absolute map value without relabeling units.
    ///
    /// # Errors
    ///
    /// Returns an error when the sigma multiplier is non-finite or the volume
    /// has invalid statistics.
    pub fn value_at_sigma(&self, sigma: f32) -> Result<f32, Error> {
        if !sigma.is_finite() {
            return Err(Error::InvalidSpec(
                "density sigma must be finite".to_owned(),
            ));
        }
        let stats = self.statistics()?;
        Ok(stats.mean + sigma * stats.sigma)
    }
}

/// Runtime bulk data a resolved scene's overlay descriptors can reach.
#[derive(Clone, Debug, Default)]
pub(crate) struct OverlayBindings {
    volumes: BTreeMap<Box<str>, VolumeBinding>,
    molecular_surfaces: BTreeMap<
        crate::StructureId,
        BTreeMap<Box<str>, crate::representation::MolecularSurfaceBinding>,
    >,
    hydrogen_bonds: BTreeMap<Box<str>, super::HydrogenBondBinding>,
    segmentations: BTreeMap<Box<str>, super::SegmentationBinding>,
    trajectories: BTreeMap<Box<str>, TrajectoryBinding>,
}

impl OverlayBindings {
    pub(crate) fn absorb(&mut self, retained: Self) -> Result<(), Error> {
        for (key, binding) in retained.volumes {
            if let Some(existing) = self.volumes.get(&key) {
                if existing.dimensions != binding.dimensions
                    || !same_bits(existing.voxel_to_world, binding.voxel_to_world)
                    || existing.values.len() != binding.values.len()
                    || !existing
                        .values
                        .iter()
                        .zip(binding.values.iter())
                        .all(|(a, b)| a.to_bits() == b.to_bits())
                {
                    return Err(conflicting_archive());
                }
            } else {
                self.volumes.insert(key, binding);
            }
        }
        for (key, binding) in retained.segmentations {
            if let Some(existing) = self.segmentations.get(&key) {
                if !existing.same_input(&binding) {
                    return Err(conflicting_archive());
                }
            } else {
                self.segmentations.insert(key, binding);
            }
        }
        for (key, binding) in retained.trajectories {
            if let Some(existing) = self.trajectories.get(&key) {
                if !existing.same_input(&binding) {
                    return Err(conflicting_archive());
                }
            } else {
                self.trajectories.insert(key, binding);
            }
        }
        for binding in retained.hydrogen_bonds.into_values() {
            self.insert_hydrogen_bonds(binding)?;
        }
        for scoped in retained.molecular_surfaces.into_values() {
            for binding in scoped.into_values() {
                self.insert_molecular_surface(binding)?;
            }
        }
        Ok(())
    }

    pub(crate) fn retain_sources(&mut self, sources: &std::collections::BTreeSet<Box<str>>) {
        self.volumes.retain(|key, _| sources.contains(key.as_ref()));
        self.segmentations
            .retain(|key, _| sources.contains(key.as_ref()));
        self.trajectories
            .retain(|key, _| sources.contains(key.as_ref()));
        self.hydrogen_bonds
            .retain(|key, _| sources.contains(key.as_ref()));
        self.molecular_surfaces.retain(|_, scoped| {
            scoped.retain(|key, _| sources.contains(key.as_ref()));
            !scoped.is_empty()
        });
    }

    pub(crate) fn insert_molecular_surface(
        &mut self,
        binding: crate::representation::MolecularSurfaceBinding,
    ) -> Result<(), Error> {
        let key = binding.source.content_hash.clone();
        let scoped = self
            .molecular_surfaces
            .entry(binding.structure)
            .or_default();
        if let Some(existing) = scoped.get(&key) {
            if existing.same_computation(&binding) {
                if existing.molecular_key != binding.molecular_key {
                    let _ = scoped.insert(key, binding);
                }
                return Ok(());
            }
            return Err(Error::InvalidSpec(format!(
                "analytical surface source '{key}' contradicts an existing computation"
            )));
        }
        let _ = scoped.insert(key, binding);
        Ok(())
    }
    pub(crate) fn molecular_surface(
        &self,
        hash: &str,
        structure: crate::StructureId,
    ) -> Option<&crate::representation::MolecularSurfaceBinding> {
        self.molecular_surfaces.get(&structure)?.get(hash)
    }

    pub(crate) fn insert_hydrogen_bonds(
        &mut self,
        binding: super::HydrogenBondBinding,
    ) -> Result<(), Error> {
        binding.validate()?;
        let key: Box<str> = binding.content_hash().into();
        if let Some(existing) = self.hydrogen_bonds.get(&key) {
            if existing.same_input(&binding) {
                return Ok(());
            }
            return Err(Error::InvalidSpec(format!(
                "hydrogen-bond source '{key}' contradicts an existing calculation input"
            )));
        }
        let _ = self.hydrogen_bonds.insert(key, binding);
        Ok(())
    }
    pub(crate) fn hydrogen_bonds(&self, hash: &str) -> Option<&super::HydrogenBondBinding> {
        self.hydrogen_bonds.get(hash)
    }

    pub(crate) fn insert_segmentation(
        &mut self,
        binding: super::SegmentationBinding,
    ) -> Result<(), Error> {
        let key: Box<str> = binding.content_hash().into();
        if self.segmentations.contains_key(&key) {
            return Err(Error::InvalidSpec(format!(
                "segmentation source '{key}' is already bound"
            )));
        }
        let _ = self.segmentations.insert(key, binding);
        Ok(())
    }
    pub(crate) fn segmentation(&self, content_hash: &str) -> Option<&super::SegmentationBinding> {
        self.segmentations.get(content_hash)
    }

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

    pub(crate) fn retain_trajectory_time(&mut self, segment: &molgfx_core::TrajectorySegment) {
        for binding in self.trajectories.values_mut() {
            binding.retain_sample_time(segment);
        }
    }
}

/// Renderer-side handle counts for each exposed overlay capability.
///
/// A portable descriptor that never reached the renderer counts zero: a density
/// volume with no runtime binding, for example, is deliberately unresolved.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct OverlayHandles {
    /// Density grids resolved against a runtime binding.
    pub volumes: usize,
    /// Categorical grids resolved against a runtime binding.
    pub segmentations: usize,
    /// Labels stored as annotations.
    pub labels: usize,
    /// Geometry-backed measurements.
    pub measurements: usize,
    /// Explicit interaction edges.
    pub interactions: usize,
    /// Structures holding a resident trajectory frame pair.
    pub trajectories: usize,
    /// Per-atom anisotropic-displacement ellipsoid overlays, with their
    /// Number of analytic anisotropic ellipsoid primitives emitted.
    pub ellipsoids: usize,
    /// SNFG symbol primitives emitted by SNFG representations.
    pub snfg_symbols: usize,
    /// Analytic guide segments emitted for caller-authored planes.
    pub plane_guides: usize,
    /// Analytic guide segments emitted for crystallographic unit cells.
    pub unit_cell_guides: usize,
}

/// Whether two coordinate triples are bit-identical.
///
/// Descriptor agreement is exact by contract; `-0.0` and `0.0` are treated as
/// different so a signed origin cannot pass unnoticed.
fn same_bits(left: [f32; 16], right: [f32; 16]) -> bool {
    left.map(f32::to_bits) == right.map(f32::to_bits)
}

fn conflicting_archive() -> Error {
    Error::InvalidSpec("retained archive contradicts an existing scientific asset".to_owned())
}
