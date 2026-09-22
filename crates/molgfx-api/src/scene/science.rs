//! Runtime scientific bindings and renderer-handle inspection.

use super::{Resolution, Scene};
use crate::error::Error;
use crate::id::StructureId;
use crate::science::{ScientificHandles, TrajectoryBinding, VolumeBinding};

impl Scene {
    /// Binds one immutable density grid for every volume descriptor naming it.
    ///
    /// The descriptor stays portable: a volume whose source has no matching
    /// binding resolves to no handle, which is the intended contract rather
    /// than an error, because the same specification may be resolved on a host
    /// that holds different bulk data.
    ///
    /// # Errors
    ///
    /// Returns an invalid-specification error for a malformed grid, a content
    /// hash that is already bound, or a grid that contradicts a descriptor it
    /// would satisfy.
    pub fn bind_volume(&mut self, binding: VolumeBinding) -> Result<(), Error> {
        self.science_bindings.insert(binding)?;
        self.rebind_science()
    }

    /// Binds one resident frame pair for every trajectory descriptor naming it.
    ///
    /// The descriptor declares how many frames a source holds; the binding
    /// supplies the two the renderer samples between. A trajectory whose source
    /// has no matching binding stays unresolved, the same contract a volume
    /// follows. The pair must match the target structure's atom count, which is
    /// checked here because the structure is resolved by the time this runs.
    ///
    /// # Errors
    ///
    /// Returns an invalid-specification error for a malformed pair, a content
    /// hash that is already bound, a pair that contradicts its descriptor, or
    /// frames that do not match the target structure's topology.
    pub fn bind_trajectory(&mut self, binding: TrajectoryBinding) -> Result<(), Error> {
        self.science_bindings.insert_trajectory(binding)?;
        self.rebind_science()
    }

    /// Advances one structure's presentation time inside its resident interval.
    ///
    /// This samples the pair already resident on the GPU, so it uploads no
    /// coordinates: only the interpolation uniform changes.
    ///
    /// # Errors
    ///
    /// Returns an invalid-specification error for a structure this scene does
    /// not carry or a time outside the resident interval.
    pub fn set_trajectory_time(
        &mut self,
        structure: StructureId,
        sample_seconds: f32,
    ) -> Result<(), Error> {
        let handle = self
            .structures
            .keys()
            .copied()
            .zip(self.resolved.structures().map(|(handle, _)| handle))
            .find(|(id, _)| *id == structure)
            .map(|(_, handle)| handle)
            .ok_or_else(|| {
                Error::InvalidSpec("trajectory time targets an unbound structure".to_owned())
            })?;
        self.resolved.set_trajectory_time(handle, sample_seconds)?;
        Ok(())
    }

    /// Re-resolves the scene so newly bound runtime data reaches the handle set.
    fn rebind_science(&mut self) -> Result<(), Error> {
        self.spec.revision = self.spec.revision.wrapping_add(1);
        let resolution = crate::scene::runtime::resolve(
            &self.spec,
            &self.structures,
            &self.property_bindings,
            &self.science_bindings,
        )?;
        self.install_resolution(resolution);
        Ok(())
    }

    /// Renderer-side handle counts for each exposed scientific capability.
    ///
    /// Scientific descriptors reach the renderer when the scene resolves, which
    /// every scientific patch, structure addition and volume binding performs.
    #[must_use]
    pub fn scientific_handles(&self) -> ScientificHandles {
        self.science.counts()
    }

    /// Replaces every resolved view of the core scene at once.
    pub(super) fn install_resolution(&mut self, resolution: Resolution) {
        self.structure_assets =
            crate::scene::runtime::StructureAssets::capture(&self.structures, &resolution.scene);
        self.resolved = resolution.scene;
        self.representations = resolution.representations;
        self.selections = resolution.selections;
        self.visuals = resolution.visuals;
        self.properties = resolution.properties;
        self.science = resolution.science;
    }
}
