//! Compact typed-array conversion at the JavaScript boundary.

use wasm_bindgen::prelude::*;

pub(super) fn scalar_ramp(values: &[f32], colors: &[u8]) -> Result<pdviewx::ScalarRamp, JsValue> {
    if values.len() != 3 || colors.len() != 12 {
        return Err(JsValue::from_str(
            "rampValues must have 3 floats and rampRgba 12 bytes",
        ));
    }
    pdviewx::ScalarRamp::new(
        [values[0], values[1], values[2]],
        [
            rgba8(&colors[0..4])?,
            rgba8(&colors[4..8])?,
            rgba8(&colors[8..12])?,
        ],
    )
    .map_err(js_error)
}

pub(super) fn rgba8(value: &[u8]) -> Result<pdviewx::Rgba8, JsValue> {
    if let [red, green, blue, alpha] = value {
        Ok(pdviewx::Rgba8::new(*red, *green, *blue, *alpha))
    } else {
        Err(JsValue::from_str("RGBA must contain exactly four bytes"))
    }
}

pub(super) fn js_error(error: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&error.to_string())
}
