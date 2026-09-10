//! Shared contiguous-array conversion helpers.

use wasm_bindgen::prelude::*;

pub(super) fn ordered_rows(namespace: u64, count: usize) -> Result<pdviewx::SourceRows, JsValue> {
    let count = u32::try_from(count).map_err(|_| js_value("row count exceeds u32"))?;
    Ok(pdviewx::SourceRows::ordered(
        pdviewx::SourceNamespace(namespace),
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

pub(super) fn rgba8(values: &[u8]) -> Result<pdviewx::Rgba8, JsValue> {
    if let [red, green, blue, alpha] = values {
        Ok(pdviewx::Rgba8::new(*red, *green, *blue, *alpha))
    } else {
        Err(js_value("rgba must contain exactly four bytes"))
    }
}

#[allow(clippy::needless_pass_by_value)]
pub(crate) fn core_error(error: pdviewx::CoreError) -> JsValue {
    js_value(&error.to_string())
}

pub(super) fn js_value(message: &str) -> JsValue {
    JsValue::from_str(message)
}
