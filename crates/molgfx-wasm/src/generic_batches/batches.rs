//! Point and rigid-instance insertion from contiguous arrays.

use super::{
    values::{core_error, js_value, ordered_rows, rgba8, vec3_rows},
    WebAnalyticTemplate, WebInstanceBatchHandle, WebPointBatchHandle,
};
use crate::browser::WebScene;
use std::sync::Arc;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
impl WebScene {
    /// Adds tightly packed xyz rows without constructing JavaScript objects.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error for malformed positions, rows or style values.
    #[wasm_bindgen(js_name = addPointBatch)]
    #[allow(clippy::needless_pass_by_value)]
    pub fn add_point_batch(
        &mut self,
        namespace: u64,
        positions: Vec<f32>,
        radius: f32,
        sphere: bool,
        rgba: Vec<u8>,
    ) -> Result<WebPointBatchHandle, JsValue> {
        let positions = vec3_rows("positions", positions)?;
        let rows = ordered_rows(namespace, positions.len())?;
        let color = rgba8(&rgba)?;
        let glyph = if sphere {
            molgfx::PointGlyph::Sphere
        } else {
            molgfx::PointGlyph::Disc
        };
        let batch = molgfx::PointBatch::new(
            Arc::from(positions),
            rows,
            glyph,
            molgfx::PointStyle { radius, color },
        )
        .map_err(core_error)?;
        Ok(WebPointBatchHandle {
            inner: self.inner.add_point_batch(batch),
        })
    }

    /// Adds 32-byte rigid transforms from flat contiguous transform columns.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error for misaligned or invalid transform rows.
    #[wasm_bindgen(js_name = addInstanceBatch)]
    #[allow(clippy::needless_pass_by_value)]
    pub fn add_instance_batch(
        &mut self,
        template: &WebAnalyticTemplate,
        namespace: u64,
        translations: Vec<f32>,
        orientations: Vec<f32>,
        scales: Vec<f32>,
        rgba: Vec<u8>,
    ) -> Result<WebInstanceBatchHandle, JsValue> {
        if !translations.len().is_multiple_of(3)
            || !orientations.len().is_multiple_of(4)
            || translations.len() / 3 != orientations.len() / 4
            || scales.len() != translations.len() / 3
        {
            return Err(js_value("instance transform columns must be row-aligned"));
        }
        let mut transforms = Vec::with_capacity(scales.len());
        for (row, scale) in scales.iter().copied().enumerate() {
            let xyz = row * 3;
            let xyzw = row * 4;
            transforms.push(
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
        let rows = ordered_rows(namespace, transforms.len())?;
        let batch =
            molgfx::InstanceBatch::new(Arc::clone(&template.inner), Arc::from(transforms), rows)
                .map_err(core_error)?
                .with_style(molgfx::InstanceStyle {
                    color: rgba8(&rgba)?,
                });
        Ok(WebInstanceBatchHandle {
            inner: self.inner.add_instance_batch(batch),
        })
    }
}
