//! Analytical surfaces use the canonical scientific-result wire schema.
use super::{WebScene, javascript_error};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(js_class = Scene)]
impl WebScene {
    /// Binds provider metadata, triangles, analytic normals and original atom rows.
    /// Native solvers run in the host; the browser renders their validated output.
    ///
    /// # Errors
    /// Rejects unresolved scenes, malformed assets and contradictory scientific controls.
    #[wasm_bindgen(js_name = bindMolecularSurface)]
    pub fn bind_molecular_surface(&mut self, identity: u64, data: &str) -> Result<(), JsError> {
        let data: molgfx::MolecularSurfaceData =
            serde_json::from_str(data).map_err(javascript_error)?;
        let scene = self
            .resolved
            .as_mut()
            .ok_or_else(|| JsError::new("resolve the scene before binding a molecular surface"))?;
        let identity = molgfx::RepresentationId::new(identity);
        let recipe = scene
            .spec()
            .representations
            .get(&identity)
            .cloned()
            .ok_or_else(|| JsError::new("unknown representation identity"))?;
        scene
            .bind_molecular_surface_data(identity, data)
            .map_err(javascript_error)?;
        let binding = scene
            .bound_molecular_surfaces()
            .find(|(id, _)| *id == identity)
            .map(|(_, binding)| binding.clone())
            .ok_or_else(|| JsError::new("bound molecular surface is unavailable"))?;
        let _ = self
            .molecular_surface_bindings
            .insert(identity, (recipe, binding));
        self.spec = scene.to_spec();
        Ok(())
    }

    /// Lists molecular representations still waiting for scientific geometry.
    ///
    /// # Errors
    /// Rejects unresolved scenes or an encoding failure.
    #[wasm_bindgen(js_name = unresolvedRepresentations)]
    pub fn unresolved_representations(&self) -> Result<String, JsError> {
        let scene = self
            .resolved
            .as_ref()
            .ok_or_else(|| JsError::new("scene is unresolved"))?;
        serde_json::to_string(&scene.unresolved_representations()).map_err(javascript_error)
    }
}
