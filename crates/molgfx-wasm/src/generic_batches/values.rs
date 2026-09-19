//! Shared contiguous-array conversion helpers.

use wasm_bindgen::prelude::*;

pub(super) fn ordered_rows(
    namespace: u64,
    count: usize,
) -> Result<molgfx::core::SourceRows, JsValue> {
    let count = u32::try_from(count).map_err(|_| js_value("row count exceeds u32"))?;
    Ok(molgfx::core::SourceRows::ordered(
        molgfx::core::SourceNamespace(namespace),
        count,
    ))
}

#[allow(clippy::needless_pass_by_value)]
pub(crate) fn vec3_rows(name: &str, values: Vec<f32>) -> Result<Vec<[f32; 3]>, JsValue> {
    if !values.len().is_multiple_of(3) {
        return Err(js_value(&format!("{name} must contain flat xyz rows")));
    }
    Ok(values
        .chunks_exact(3)
        .map(|row| [row[0], row[1], row[2]])
        .collect())
}

pub(super) fn rgba8(values: &[u8]) -> Result<molgfx::math::Rgba8, JsValue> {
    if let [red, green, blue, alpha] = values {
        Ok(molgfx::math::Rgba8::new(*red, *green, *blue, *alpha))
    } else {
        Err(js_value("rgba must contain exactly four bytes"))
    }
}

#[allow(clippy::needless_pass_by_value)]
pub(crate) fn core_error(error: molgfx::core::CoreError) -> JsValue {
    js_value(&error.to_string())
}

pub(super) fn js_value(message: &str) -> JsValue {
    JsValue::from_str(message)
}
