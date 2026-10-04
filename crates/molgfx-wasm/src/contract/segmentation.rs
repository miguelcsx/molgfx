//! Browser adapters for categorical-grid authoring and label ingestion.

use super::{WebScene, javascript_error};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(js_class = Scene)]
impl WebScene {
    /// Adds a canonical categorical-grid specification and returns its identity JSON.
    ///
    /// # Errors
    /// Returns an error for malformed metadata or an unresolved scene.
    #[wasm_bindgen(js_name = addSegmentation)]
    pub fn add_segmentation(&mut self, specification: &str) -> Result<String, JsError> {
        let spec: molgfx::SegmentationSpec =
            serde_json::from_str(specification).map_err(javascript_error)?;
        let scene = self
            .resolved
            .as_mut()
            .ok_or_else(|| JsError::new("resolve the scene before adding a segmentation"))?;
        let id = scene.add_segmentation(spec).map_err(javascript_error)?;
        self.spec = scene.to_spec();
        serde_json::to_string(&id).map_err(javascript_error)
    }

    /// Binds exact unsigned labels to a generational categorical-grid identity.
    ///
    /// # Errors
    /// Returns an error for unknown or stale identities, invalid grids, or an unresolved scene.
    #[wasm_bindgen(js_name = bindSegmentation)]
    pub fn bind_segmentation(
        &mut self,
        index: u64,
        generation: u64,
        labels: Vec<u32>,
    ) -> Result<(), JsError> {
        let id = molgfx::SegmentationId { index, generation };
        let scene = self
            .resolved
            .as_mut()
            .ok_or_else(|| JsError::new("resolve the scene before binding a segmentation"))?;
        let spec = scene
            .spec()
            .segmentations
            .get(&id)
            .ok_or_else(|| JsError::new("unknown segmentation identity"))?;
        let content_hash = spec.source.content_hash.clone();
        let binding = molgfx::SegmentationBinding::new(
            spec.source.clone(),
            spec.dimensions,
            spec.voxel_to_world,
            std::sync::Arc::from(labels),
        )
        .map_err(javascript_error)?;
        scene
            .bind_segmentation(binding.clone())
            .map_err(javascript_error)?;
        let _ = self.segmentation_bindings.insert(content_hash, binding);
        self.spec = scene.to_spec();
        Ok(())
    }

    /// Replaces the categorical style table from canonical JSON.
    ///
    /// # Errors
    /// Returns an error for malformed styles, unknown identities, or an unresolved scene.
    #[wasm_bindgen(js_name = setSegmentStyles)]
    pub fn set_segment_styles(
        &mut self,
        index: u64,
        generation: u64,
        styles: &str,
    ) -> Result<(), JsError> {
        let styles: Vec<molgfx::SegmentStyle> =
            serde_json::from_str(styles).map_err(javascript_error)?;
        let scene = self
            .resolved
            .as_mut()
            .ok_or_else(|| JsError::new("resolve the scene before changing segment styles"))?;
        scene
            .set_segment_styles(molgfx::SegmentationId { index, generation }, styles)
            .map_err(javascript_error)?;
        self.spec = scene.to_spec();
        Ok(())
    }

    /// Removes one exact categorical-grid identity.
    ///
    /// # Errors
    /// Returns an error for unknown identities or an unresolved scene.
    #[wasm_bindgen(js_name = removeSegmentation)]
    pub fn remove_segmentation(&mut self, index: u64, generation: u64) -> Result<(), JsError> {
        let scene = self
            .resolved
            .as_mut()
            .ok_or_else(|| JsError::new("resolve the scene before removing a segmentation"))?;
        let id = molgfx::SegmentationId { index, generation };
        let content_hash = scene
            .spec()
            .segmentations
            .get(&id)
            .ok_or_else(|| JsError::new("unknown segmentation identity"))?
            .source
            .content_hash
            .clone();
        scene
            .apply(&molgfx::ScenePatch {
                base_revision: scene.revision(),
                operations: vec![molgfx::schema::PatchOperation::RemoveSegmentation { id }],
            })
            .map_err(javascript_error)?;
        if !scene
            .spec()
            .segmentations
            .values()
            .any(|spec| spec.source.content_hash == content_hash)
        {
            let _ = self.segmentation_bindings.remove(&content_hash);
        }
        self.spec = scene.to_spec();
        Ok(())
    }
}

#[cfg(test)]
#[path = "segmentation_tests.rs"]
mod tests;
