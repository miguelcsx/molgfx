//! Transient pointer state shared with the native renderer.
use super::{WebScene, javascript_error};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(js_class = Scene)]
impl WebScene {
    /// Highlights resolved pick JSON without changing the portable scene.
    /// Omit the pick to restore the authored hover channel.
    ///
    /// # Errors
    /// Returns an error for malformed, stale or unresolved picks.
    #[wasm_bindgen(js_name = setPointerHover)]
    pub fn set_pointer_hover(&mut self, pick: Option<String>) -> Result<bool, JsError> {
        let pick: Option<molgfx::ResolvedPick> = pick
            .map(|source| serde_json::from_str(&source))
            .transpose()
            .map_err(javascript_error)?;
        self.resolved
            .as_mut()
            .ok_or_else(|| JsError::new("scene must be resolved before hover updates"))?
            .set_pointer_hover(pick.as_ref())
            .map_err(javascript_error)
    }
}
