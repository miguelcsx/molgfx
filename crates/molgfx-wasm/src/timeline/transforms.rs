//! Contiguous transform-column validation and packing.

use crate::generic_batches::core_error;
use wasm_bindgen::prelude::*;

pub(super) fn rigid_rows(
    translations: &[f32],
    orientations: &[f32],
    scales: Vec<f32>,
) -> Result<Vec<molgfx::RigidInstance>, JsValue> {
    if !translations.len().is_multiple_of(3)
        || !orientations.len().is_multiple_of(4)
        || translations.len() / 3 != orientations.len() / 4
        || scales.len() != translations.len() / 3
    {
        return Err(JsValue::from_str(
            "timeline transform columns must be row-aligned",
        ));
    }
    let mut rows = Vec::with_capacity(scales.len());
    for (row, scale) in scales.into_iter().enumerate() {
        let xyz = row * 3;
        let xyzw = row * 4;
        rows.push(
            molgfx::RigidInstance::new(
                molgfx::Vec3::new(
                    translations[xyz],
                    translations[xyz + 1],
                    translations[xyz + 2],
                ),
                molgfx::Quat::from_array([
                    orientations[xyzw],
                    orientations[xyzw + 1],
                    orientations[xyzw + 2],
                    orientations[xyzw + 3],
                ]),
                scale,
            )
            .map_err(core_error)?,
        );
    }
    Ok(rows)
}
