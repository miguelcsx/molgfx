//! Browser handles and exact row-domain identities.

use wasm_bindgen::prelude::*;

macro_rules! web_handle {
    ($name:ident, $native:ident) => {
        #[doc = concat!("Generational browser handle for `", stringify!($native), "`.")]
        #[wasm_bindgen]
        #[derive(Clone, Copy, Debug)]
        pub struct $name {
            pub(crate) inner: pdviewx::$native,
        }

        #[wasm_bindgen]
        impl $name {
            /// Stable slot row within the current scene generation.
            #[must_use]
            #[wasm_bindgen(getter)]
            pub fn row(&self) -> u32 {
                self.inner.row()
            }

            /// Generation that prevents stale-handle reuse.
            #[must_use]
            #[wasm_bindgen(getter)]
            pub fn generation(&self) -> u32 {
                self.inner.generation()
            }
        }
    };
}

web_handle!(WebAttributeHandle, AttributeHandle);
web_handle!(WebPointBatchHandle, PointBatchHandle);
web_handle!(WebInstanceBatchHandle, InstanceBatchHandle);
web_handle!(WebRelationBatchHandle, RelationBatchHandle);

/// Exact homogeneous table targeted by an attribute or dynamic anchor.
#[wasm_bindgen]
#[derive(Clone, Copy, Debug)]
pub struct WebRowDomain {
    pub(crate) inner: pdviewx::RowDomain,
}

#[wasm_bindgen]
impl WebRowDomain {
    /// Targets rows in one generic point batch.
    #[must_use]
    #[wasm_bindgen(js_name = points)]
    pub fn points(handle: &WebPointBatchHandle) -> Self {
        Self {
            inner: pdviewx::RowDomain::Points(handle.inner),
        }
    }

    /// Targets rigid transform rows in one instance batch.
    #[must_use]
    #[wasm_bindgen(js_name = instances)]
    pub fn instances(handle: &WebInstanceBatchHandle) -> Self {
        Self {
            inner: pdviewx::RowDomain::Instances(handle.inner),
        }
    }

    /// Targets shared analytic parts in one instance template.
    #[must_use]
    #[wasm_bindgen(js_name = templateParts)]
    pub fn template_parts(handle: &WebInstanceBatchHandle) -> Self {
        Self {
            inner: pdviewx::RowDomain::TemplateParts(handle.inner),
        }
    }

    /// Targets rows in one generic relation batch.
    #[must_use]
    #[wasm_bindgen(js_name = relations)]
    pub fn relations(handle: &WebRelationBatchHandle) -> Self {
        Self {
            inner: pdviewx::RowDomain::Relations(handle.inner),
        }
    }
}
