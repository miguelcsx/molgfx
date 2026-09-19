//! Browser-visible result metadata.

use wasm_bindgen::prelude::*;

/// Lightweight result metadata for one declarative composition.
#[wasm_bindgen]
#[derive(Clone, Debug)]
pub struct WebGenericCompositionView {
    inner: molgfx::semantic::GenericCompositionView,
}

impl From<molgfx::semantic::GenericCompositionView> for WebGenericCompositionView {
    fn from(inner: molgfx::semantic::GenericCompositionView) -> Self {
        Self { inner }
    }
}

#[wasm_bindgen]
impl WebGenericCompositionView {
    /// Number of styled homogeneous domains.
    #[must_use]
    #[wasm_bindgen(getter, js_name = domainCount)]
    pub fn domain_count(&self) -> u32 {
        u32::try_from(self.inner.domains.len()).map_or(u32::MAX, |count| count)
    }

    /// Normalized ensemble weights in input order, or an empty array.
    #[must_use]
    #[wasm_bindgen(getter, js_name = normalizedWeights)]
    pub fn normalized_weights(&self) -> Vec<f32> {
        self.inner.normalized_weights.clone()
    }
}
