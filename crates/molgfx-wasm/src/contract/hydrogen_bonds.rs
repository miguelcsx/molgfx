//! Bulk interaction addresses share the canonical scene binding contract.

use super::{WebScene, javascript_error};
use std::sync::Arc;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(js_class = Scene)]
impl WebScene {
    /// Binds flattened donor, explicit hydrogen and acceptor atom-row triples.
    ///
    /// Classification belongs to the caller's molecular analysis. The GPU uses
    /// these addresses to follow live coordinates without receiving snapshots.
    ///
    /// # Errors
    /// Rejects malformed triples, unknown presentations and invalid atom rows.
    #[wasm_bindgen(js_name = bindHydrogenBonds)]
    pub fn bind_hydrogen_bonds(
        &mut self,
        identity: u64,
        endpoints: &[u32],
        candidate_distance: Option<f32>,
    ) -> Result<(), JsError> {
        if !endpoints.len().is_multiple_of(3) {
            return Err(JsError::new("hydrogen-bond addresses require triples"));
        }
        let scene = self
            .resolved
            .as_mut()
            .ok_or_else(|| JsError::new("resolve the scene before binding hydrogen bonds"))?;
        let Some(molgfx::InteractionSpec::HydrogenBonds(spec)) = scene
            .spec()
            .interactions
            .get(&molgfx::InteractionId::new(identity))
        else {
            return Err(JsError::new(
                "identity is not a hydrogen-bond table presentation",
            ));
        };
        let source = spec.source.clone();
        let rows: Arc<[[u32; 3]]> = Arc::from(endpoints.as_chunks::<3>().0);
        let binding = match candidate_distance {
            Some(distance) => {
                molgfx::HydrogenBondBinding::candidates(source.clone(), rows, distance)
            }
            None => molgfx::HydrogenBondBinding::new(source.clone(), rows),
        };
        scene
            .bind_hydrogen_bonds(binding.clone())
            .map_err(javascript_error)?;
        let binding = scene
            .bound_hydrogen_bonds()
            .find(|(_, binding)| binding.source().content_hash == source.content_hash)
            .map(|(_, binding)| binding.clone())
            .ok_or_else(|| JsError::new("bound hydrogen-bond input is unavailable"))?;
        let _ = self
            .hydrogen_bond_bindings
            .insert(source.content_hash, binding);
        self.spec = scene.to_spec();
        Ok(())
    }
}
