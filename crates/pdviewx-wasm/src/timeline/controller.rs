//! Allocation-free timeline controller after batch tracks are bound.

use super::{WebTimeWarp, WebTimelineTrackHandle, transforms::rigid_rows};
use crate::browser::WebScene;
use crate::generic_batches::{
    WebAttributeHandle, WebInstanceBatchHandle, WebPointBatchHandle, core_error, vec3_rows,
};
use std::sync::Arc;
use wasm_bindgen::prelude::*;

/// Allocation-free timeline controller after batch tracks are bound.
#[wasm_bindgen]
#[derive(Debug)]
pub struct WebTimeline {
    inner: pdviewx::Timeline,
}

#[wasm_bindgen]
impl WebTimeline {
    /// Creates an empty timeline controller.
    #[must_use]
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            inner: pdviewx::Timeline::new(),
        }
    }

    /// Binds two tightly packed xyz point frames.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error for malformed arrays or stale scene handles.
    #[wasm_bindgen(js_name = bindPoints)]
    #[allow(clippy::needless_pass_by_value)]
    pub fn bind_points(
        &mut self,
        scene: &mut WebScene,
        batch: &WebPointBatchHandle,
        start: Vec<f32>,
        end: Vec<f32>,
        warp: &WebTimeWarp,
    ) -> Result<WebTimelineTrackHandle, JsValue> {
        let start = vec3_rows("start", start)?;
        let end = vec3_rows("end", end)?;
        self.inner
            .bind_points(
                &mut scene.inner,
                batch.inner,
                Arc::from(start),
                Arc::from(end),
                warp.inner,
            )
            .map(|inner| WebTimelineTrackHandle { inner })
            .map_err(core_error)
    }

    /// Binds two rigid-transform frames using flat `SoA` columns.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error for malformed arrays or stale scene handles.
    #[wasm_bindgen(js_name = bindInstances)]
    #[allow(clippy::too_many_arguments, clippy::needless_pass_by_value)]
    pub fn bind_instances(
        &mut self,
        scene: &mut WebScene,
        batch: &WebInstanceBatchHandle,
        start_translations: Vec<f32>,
        start_orientations: Vec<f32>,
        start_scales: Vec<f32>,
        end_translations: Vec<f32>,
        end_orientations: Vec<f32>,
        end_scales: Vec<f32>,
        warp: &WebTimeWarp,
    ) -> Result<WebTimelineTrackHandle, JsValue> {
        let start = rigid_rows(&start_translations, &start_orientations, start_scales)?;
        let end = rigid_rows(&end_translations, &end_orientations, end_scales)?;
        self.inner
            .bind_instances(
                &mut scene.inner,
                batch.inner,
                Arc::from(start),
                Arc::from(end),
                warp.inner,
            )
            .map(|inner| WebTimelineTrackHandle { inner })
            .map_err(core_error)
    }

    /// Binds two native-width scalar attribute frames.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error for mismatched rows or a stale attribute.
    #[wasm_bindgen(js_name = bindScalarAttribute)]
    pub fn bind_scalar_attribute(
        &mut self,
        scene: &mut WebScene,
        attribute: &WebAttributeHandle,
        start: Vec<f32>,
        end: Vec<f32>,
        warp: &WebTimeWarp,
    ) -> Result<WebTimelineTrackHandle, JsValue> {
        self.bind_attribute(
            scene,
            attribute,
            pdviewx::AttributeValues::Scalar(Arc::from(start)),
            pdviewx::AttributeValues::Scalar(Arc::from(end)),
            warp,
        )
    }

    /// Binds two tightly packed xyz vector attribute frames.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error for malformed rows or a stale attribute.
    #[wasm_bindgen(js_name = bindVectorAttribute)]
    #[allow(clippy::needless_pass_by_value)]
    pub fn bind_vector_attribute(
        &mut self,
        scene: &mut WebScene,
        attribute: &WebAttributeHandle,
        start: Vec<f32>,
        end: Vec<f32>,
        warp: &WebTimeWarp,
    ) -> Result<WebTimelineTrackHandle, JsValue> {
        self.bind_attribute(
            scene,
            attribute,
            pdviewx::AttributeValues::Vector(Arc::from(vec3_rows("start", start)?)),
            pdviewx::AttributeValues::Vector(Arc::from(vec3_rows("end", end)?)),
            warp,
        )
    }

    /// Applies all tracks for one presentation timestamp.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error if any bound scene target became stale.
    pub fn apply(&mut self, scene: &mut WebScene, global_seconds: f64) -> Result<(), JsValue> {
        self.inner
            .apply(&mut scene.inner, global_seconds)
            .map_err(core_error)
    }

    /// Removes a track and releases its scene-side temporal source.
    pub fn remove(&mut self, scene: &mut WebScene, handle: &WebTimelineTrackHandle) -> bool {
        self.inner.unbind(&mut scene.inner, handle.inner)
    }

    /// Number of currently bound tracks.
    #[must_use]
    #[wasm_bindgen(getter)]
    pub fn length(&self) -> usize {
        self.inner.len()
    }

    fn bind_attribute(
        &mut self,
        scene: &mut WebScene,
        attribute: &WebAttributeHandle,
        start: pdviewx::AttributeValues,
        end: pdviewx::AttributeValues,
        warp: &WebTimeWarp,
    ) -> Result<WebTimelineTrackHandle, JsValue> {
        self.inner
            .bind_attribute(&mut scene.inner, attribute.inner, start, end, warp.inner)
            .map(|inner| WebTimelineTrackHandle { inner })
            .map_err(core_error)
    }
}

impl Default for WebTimeline {
    fn default() -> Self {
        Self::new()
    }
}
