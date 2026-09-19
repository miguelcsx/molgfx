//! Browser builder for caller-colored weighted overlays.

use super::{WebGenericCompositionView, values};
use crate::browser::WebScene;
use crate::generic_batches::WebRowDomain;
use wasm_bindgen::prelude::*;

/// Reusable builder for a caller-colored weighted overlay.
#[wasm_bindgen]
#[derive(Clone, Debug)]
pub struct WebEnsembleComposition {
    layers: Vec<molgfx::semantic::EnsembleLayer>,
    style: molgfx::semantic::EnsembleCompositionStyle,
}

#[wasm_bindgen]
impl WebEnsembleComposition {
    /// Creates the generic opacity policy; layers are added as compact metadata.
    #[must_use]
    #[wasm_bindgen(constructor)]
    pub fn new(
        dominant_opacity: f32,
        alternate_opacity: f32,
        minimum_opacity: f32,
        order: i32,
    ) -> Self {
        Self {
            layers: Vec::new(),
            style: molgfx::semantic::EnsembleCompositionStyle {
                dominant_opacity,
                alternate_opacity,
                minimum_opacity,
                order,
            },
        }
    }

    /// Appends one weighted domain. RGBA must contain exactly four bytes.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error unless `rgba` contains exactly four bytes.
    #[wasm_bindgen(js_name = addLayer)]
    #[allow(clippy::needless_pass_by_value)]
    pub fn add_layer(
        &mut self,
        domain: &WebRowDomain,
        weight: f32,
        rgba: Vec<u8>,
    ) -> Result<(), JsValue> {
        self.layers.push(molgfx::semantic::EnsembleLayer {
            domain: domain.inner,
            weight,
            color: values::rgba8(&rgba)?,
        });
        Ok(())
    }

    /// Validates every layer before atomically attaching the visual descriptors.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error for stale domains or invalid weights and opacities.
    pub fn apply(&self, scene: &mut WebScene) -> Result<WebGenericCompositionView, JsValue> {
        molgfx::semantic::GenericCompositionScene::compose_ensemble(
            &mut scene.inner,
            &self.layers,
            self.style,
        )
        .map(Into::into)
        .map_err(values::js_error)
    }
}
