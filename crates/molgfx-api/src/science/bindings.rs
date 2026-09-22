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

/// Runtime bulk data a resolved scene's scientific descriptors can reach.
#[derive(Clone, Debug, Default)]
pub(crate) struct ScienceBindings {
    volumes: BTreeMap<Box<str>, VolumeBinding>,
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
}

/// Whether two coordinate triples are bit-identical.
///
/// Descriptor agreement is exact by contract; `-0.0` and `0.0` are treated as
/// different so a signed origin cannot pass unnoticed.
fn same_bits(left: [f32; 3], right: [f32; 3]) -> bool {
    left.map(f32::to_bits) == right.map(f32::to_bits)
}
