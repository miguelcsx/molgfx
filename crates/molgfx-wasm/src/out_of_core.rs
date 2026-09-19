//! Out-of-core identities and byte charges for the browser surface.
//!
//! Each identity wraps the matching native type so a JavaScript caller can hold
//! and compare a complete `u64` or `u32` without narrowing, and a chunk
//! footprint reports what one chunk costs without transferring its payload.

use wasm_bindgen::prelude::*;

macro_rules! web_id {
    ($web:ident, $module:ident, $native:ident, $value:ty) => {
        /// Portable global or chunk-local identity without narrowing.
        #[wasm_bindgen]
        #[derive(Clone, Copy, Debug)]
        pub struct $web(molgfx::$module::$native);

        #[wasm_bindgen]
        impl $web {
            /// Creates the identity from its complete integer value.
            #[wasm_bindgen(constructor)]
            #[must_use]
            pub fn new(value: $value) -> Self {
                Self(molgfx::$module::$native::new(value))
            }

            /// Returns the complete identity value.
            #[wasm_bindgen(getter)]
            #[must_use]
            pub fn value(&self) -> $value {
                self.0.get()
            }
        }
    };
}

web_id!(WebDatasetId, semantic, DatasetId, u64);
web_id!(WebChunkId, semantic, ChunkId, u64);
web_id!(WebLogicalRow, semantic, LogicalRow, u64);
web_id!(WebLocalRow, core, LocalRow, u32);

/// Byte charges for one chunk without transferring its payload.
#[wasm_bindgen]
#[derive(Clone, Copy, Debug)]
pub struct WebChunkFootprint(molgfx::semantic::ChunkFootprint);

#[wasm_bindgen]
impl WebChunkFootprint {
    /// Creates metadata only; no payload buffer is copied.
    #[wasm_bindgen(constructor)]
    #[must_use]
    pub fn new(source_bytes: u64, host_bytes: u64, staging_bytes: u64, gpu_bytes: u64) -> Self {
        Self(molgfx::semantic::ChunkFootprint::new(
            source_bytes,
            host_bytes,
            staging_bytes,
            gpu_bytes,
        ))
    }

    /// Returns the `source_bytes` charge.
    #[wasm_bindgen(getter, js_name = sourceBytes)]
    #[must_use]
    pub fn source_bytes(&self) -> u64 {
        self.0.source_bytes
    }

    /// Returns the `host_bytes` charge.
    #[wasm_bindgen(getter, js_name = hostBytes)]
    #[must_use]
    pub fn host_bytes(&self) -> u64 {
        self.0.host_bytes
    }

    /// Returns the `staging_bytes` charge.
    #[wasm_bindgen(getter, js_name = stagingBytes)]
    #[must_use]
    pub fn staging_bytes(&self) -> u64 {
        self.0.staging_bytes
    }

    /// Returns the `gpu_bytes` charge.
    #[wasm_bindgen(getter, js_name = gpuBytes)]
    #[must_use]
    pub fn gpu_bytes(&self) -> u64 {
        self.0.gpu_bytes
    }

    /// Returns the checked total charge.
    ///
    /// # Errors
    /// Returns a JavaScript error when the sum exceeds `u64`.
    #[wasm_bindgen(js_name = totalBytes)]
    pub fn total_bytes(&self) -> Result<u64, JsValue> {
        self.0
            .total_bytes()
            .map_err(|error| JsValue::from_str(&error.to_string()))
    }
}
