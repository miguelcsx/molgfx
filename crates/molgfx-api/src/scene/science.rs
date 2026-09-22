//! Runtime scientific bindings and renderer-handle inspection.

use super::{Resolution, Scene};
use crate::error::Error;
use crate::science::{ScientificHandles, VolumeBinding};

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
        self.resolved = resolution.scene;
        self.representations = resolution.representations;
        self.selections = resolution.selections;
        self.visuals = resolution.visuals;
        self.properties = resolution.properties;
        self.science = resolution.science;
    }
}
