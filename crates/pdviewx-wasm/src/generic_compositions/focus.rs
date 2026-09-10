//! Browser entry point for a scalar focus/context composition.

use super::{WebGenericCompositionView, values};
use crate::browser::WebScene;
use crate::generic_batches::{WebAttributeHandle, WebRowDomain};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
impl WebScene {
    /// Applies a scalar emphasis column as a focus/context visual composition.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error for stale domains or invalid visual metadata.
    #[wasm_bindgen(js_name = composeFocus)]
    pub fn compose_focus(
        &mut self,
        domain: &WebRowDomain,
        emphasis: &WebAttributeHandle,
        context_opacity: f32,
        focus_opacity: f32,
        order: i32,
    ) -> Result<WebGenericCompositionView, JsValue> {
        pdviewx::GenericCompositionScene::compose_focus(
            &mut self.inner,
            pdviewx::FocusLayer {
                domain: domain.inner,
                emphasis: emphasis.inner,
            },
            pdviewx::FocusCompositionStyle {
                context_opacity,
                focus_opacity,
                order,
            },
        )
        .map(Into::into)
        .map_err(values::js_error)
    }
}
