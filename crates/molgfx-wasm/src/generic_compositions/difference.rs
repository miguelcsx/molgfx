//! Browser builder for multi-domain scalar differences.

use super::{WebGenericCompositionView, values};
use crate::browser::WebScene;
use crate::generic_batches::{WebAttributeHandle, WebRowDomain};
use wasm_bindgen::prelude::*;

/// Reusable builder for a multi-domain scalar-difference overlay.
#[wasm_bindgen]
#[derive(Clone, Debug)]
pub struct WebDifferenceComposition {
    layers: Vec<molgfx::semantic::DifferenceLayer>,
    style: molgfx::semantic::DifferenceCompositionStyle,
}

#[wasm_bindgen]
impl WebDifferenceComposition {
    /// Creates a visual-only difference policy from three ramp stops.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error when the ramp arrays are malformed.
    #[wasm_bindgen(constructor)]
    #[allow(clippy::needless_pass_by_value)]
    pub fn new(
        context_threshold: f32,
        emphasis_threshold: f32,
        ramp_values: Vec<f32>,
        ramp_rgba: Vec<u8>,
        context_opacity: f32,
        order: i32,
    ) -> Result<WebDifferenceComposition, JsValue> {
        let ramp = values::scalar_ramp(&ramp_values, &ramp_rgba)?;
        Ok(Self {
            layers: Vec::new(),
            style: molgfx::semantic::DifferenceCompositionStyle {
                context_threshold,
                emphasis_threshold,
                context_opacity,
                ramp,
                order,
            },
        })
    }

    /// Appends one domain/attribute pair; this is composition metadata, not per-row data.
    #[wasm_bindgen(js_name = addLayer)]
    pub fn add_layer(&mut self, domain: &WebRowDomain, delta: &WebAttributeHandle) {
        self.layers.push(molgfx::semantic::DifferenceLayer {
            domain: domain.inner,
            delta: delta.inner,
        });
    }

    /// Validates every layer before atomically attaching the visual descriptors.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error for stale domains or invalid visual metadata.
    pub fn apply(&self, scene: &mut WebScene) -> Result<WebGenericCompositionView, JsValue> {
        molgfx::semantic::GenericCompositionScene::compose_difference(
            &mut scene.inner,
            &self.layers,
            self.style,
        )
        .map(Into::into)
        .map_err(values::js_error)
    }
}
