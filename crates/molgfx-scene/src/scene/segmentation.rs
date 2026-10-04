//! Authoring lifecycle and content-safe resolution for categorical grids.

use super::Scene;
use crate::{
    Error, PatchOperation, ScenePatch, SegmentStyle, SegmentationBinding, SegmentationId,
    SegmentationSpec,
};

impl Scene {
    /// Adds a categorical descriptor with a fresh generation for its domain slot.
    ///
    /// # Errors
    /// Returns invalid descriptor, generation exhaustion or resolution errors.
    pub fn add_segmentation(
        &mut self,
        segmentation: SegmentationSpec,
    ) -> Result<SegmentationId, Error> {
        let mut transaction = self.begin();
        let id = transaction.add_segmentation(segmentation)?;
        let _ = self.commit(transaction)?;
        Ok(id)
    }

    /// Removes exactly the specified categorical-grid lifetime.
    ///
    /// # Errors
    /// Returns missing-ID or resolution errors; a stale generation never removes another grid.
    pub fn remove_segmentation(&mut self, id: SegmentationId) -> Result<(), Error> {
        self.apply(&ScenePatch {
            base_revision: self.revision(),
            operations: vec![PatchOperation::RemoveSegmentation { id }],
        })
    }

    /// Replaces only the display table, leaving immutable labels resident.
    ///
    /// # Errors
    /// Returns missing-ID or invalid-style errors without changing the scene.
    pub fn set_segment_styles(
        &mut self,
        id: SegmentationId,
        styles: Vec<SegmentStyle>,
    ) -> Result<(), Error> {
        self.apply(&ScenePatch {
            base_revision: self.revision(),
            operations: vec![PatchOperation::SetSegmentStyles { id, styles }],
        })
    }

    /// Binds shared categorical labels atomically against every matching descriptor.
    ///
    /// # Errors
    /// Returns duplicate-source, descriptor-mismatch or resolution errors. Failed binding leaves
    /// both authored state and the renderer resolution unchanged.
    pub fn bind_segmentation(&mut self, binding: SegmentationBinding) -> Result<(), Error> {
        let mut bindings = self.overlay_bindings.clone();
        bindings.insert_segmentation(binding)?;
        let mut spec = self.spec.clone();
        spec.revision = spec
            .revision
            .checked_add(1)
            .ok_or_else(|| Error::InvalidSpec("scene revisions exhausted".into()))?;
        let resolution = super::runtime::resolve_reusing(
            &spec,
            &self.structures,
            &self.property_bindings,
            &bindings,
            &self.rows,
            Some(&self.structure_assets),
        )?;
        self.spec = spec;
        self.overlay_bindings = bindings;
        self.install_resolution(resolution);
        Ok(())
    }
}
