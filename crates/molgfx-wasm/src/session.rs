//! The authoring command language in the browser.
//!
//! This is the same grammar and session the Python binding uses, compiled to
//! WebAssembly, so a page can accept commands without a kernel. Results and
//! errors cross as JSON: a failed command is an ordinary answer for a console
//! to show, not an exception.

use crate::contract::{WebScene, javascript_error};
use molgfx::command::{CommandErrors, Program, Session};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(js_name = Session)]
#[derive(Debug)]
/// Names, colour rules and history for authoring one resolved scene.
pub struct WebSession {
    inner: Session,
}

#[wasm_bindgen(js_class = Session)]
impl WebSession {
    /// Starts a session over a resolved scene.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error when the scene has not been resolved.
    #[wasm_bindgen(constructor)]
    pub fn new(scene: &WebScene) -> Result<WebSession, JsError> {
        let resolved = scene
            .resolved
            .as_ref()
            .ok_or_else(|| JsError::new("resolve the scene before starting a session"))?;
        Ok(Self {
            inner: Session::new(resolved),
        })
    }

    /// Executes command text as one atomic edit of `scene`.
    ///
    /// The answer is JSON: `{"ok": true, "patch": …, "revision": …,
    /// "messages": […]}` or `{"ok": false, "errors": […], "rendered": "…"}`.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error only when the scene is unresolved or the
    /// answer cannot be encoded.
    pub fn execute(&mut self, scene: &mut WebScene, text: &str) -> Result<String, JsError> {
        let resolved = scene
            .resolved
            .as_mut()
            .ok_or_else(|| JsError::new("resolve the scene before executing commands"))?;
        let answer =
            match Program::parse(text).and_then(|program| self.inner.execute(resolved, &program)) {
                Ok(outcome) => serde_json::json!({
                    "ok": true,
                    "patch": outcome.patch,
                    "revision": outcome.revision,
                    "messages": outcome.messages,
                    "history": self.inner.history(),
                }),
                Err(errors) => failure(&errors, text),
            };
        scene.spec = resolved.spec().clone();
        serde_json::to_string(&answer).map_err(javascript_error)
    }

    /// Completion candidates for the word ending at `cursor`, as JSON.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error when the answer cannot be encoded.
    pub fn completions(&self, text: &str, cursor: usize) -> Result<String, JsError> {
        serde_json::to_string(&self.inner.completions(text, cursor)).map_err(javascript_error)
    }

    /// Labels of the edits undo would reverse, oldest first, as JSON.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error when the answer cannot be encoded.
    pub fn history(&self) -> Result<String, JsError> {
        serde_json::to_string(&self.inner.history()).map_err(javascript_error)
    }

    /// The session's names as JSON.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error when the state cannot be encoded.
    #[wasm_bindgen(js_name = toJSON)]
    pub fn to_json(&self) -> Result<String, JsError> {
        self.inner.spec().to_json().map_err(javascript_error)
    }
}

fn failure(errors: &CommandErrors, text: &str) -> serde_json::Value {
    serde_json::json!({
        "ok": false,
        "errors": errors,
        "rendered": errors.render(text),
    })
}
