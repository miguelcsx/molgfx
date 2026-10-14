//! Camera paths and bookmarks over the same playback seconds as the timeline.
//!
//! A path is sampled with the playback clock that drives `Timeline.apply`, so
//! one number moves both the molecule and the camera. `sampleInto` writes the
//! eye, target and up vectors into caller buffers shaped for `renderCamera`.

use crate::camera::WebCamera;
use crate::contract::{javascript_error, vector3};
use crate::timeline_wire::{Easing, parse_easing};
use molgfx::camera::{CameraBookmark, CameraEasing, CameraPath};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(js_name = CameraPathBuilder)]
#[derive(Clone, Debug)]
/// Collects keyframes in playback seconds, then validates them into a path.
pub struct WebCameraPathBuilder {
    easing: CameraEasing,
    keyframes: Vec<(f64, molgfx::Camera)>,
}

#[wasm_bindgen(js_class = CameraPathBuilder)]
impl WebCameraPathBuilder {
    /// Starts an empty path; `easing` is `linear` or `smoothstep`.
    ///
    /// # Errors
    /// Returns an error for an unknown easing name.
    #[wasm_bindgen(constructor)]
    pub fn new(easing: &str) -> Result<WebCameraPathBuilder, JsError> {
        Ok(Self {
            easing: match parse_easing(easing).map_err(javascript_error)? {
                Easing::Linear => CameraEasing::Linear,
                Easing::SmoothStep => CameraEasing::SmoothStep,
            },
            keyframes: Vec::new(),
        })
    }

    /// Appends a camera at a playback timestamp. The camera stays usable.
    ///
    /// # Errors
    /// Returns an error for a non-finite time.
    pub fn keyframe(&mut self, seconds: f64, camera: &WebCamera) -> Result<(), JsError> {
        if !seconds.is_finite() {
            return Err(JsError::new("camera keyframe time must be finite"));
        }
        self.keyframes.push((seconds, camera.inner));
        Ok(())
    }

    /// Validates the keyframes into a path.
    ///
    /// # Errors
    /// Returns an error for fewer than two keyframes, timestamps that do not
    /// strictly increase, mixed projection models or a degenerate camera.
    pub fn build(&self) -> Result<WebCameraPath, JsError> {
        molgfx::camera::path(&self.keyframes, self.easing)
            .map(|inner| WebCameraPath { inner })
            .map_err(javascript_error)
    }
}

#[wasm_bindgen(js_name = CameraPath)]
#[derive(Clone, Debug)]
/// A validated camera path sampled by playback seconds.
pub struct WebCameraPath {
    inner: CameraPath,
}

#[wasm_bindgen(js_class = CameraPath)]
impl WebCameraPath {
    /// Samples the path at a playback timestamp, clamped to its endpoints.
    ///
    /// # Errors
    /// Returns an error for a non-finite time.
    pub fn sample(&self, seconds: f64) -> Result<WebCamera, JsError> {
        self.inner
            .sample(seconds)
            .map(|inner| WebCamera { inner })
            .ok_or_else(|| JsError::new("camera path time must be finite"))
    }

    /// Samples into three caller-owned buffers of three values each (eye,
    /// target, up), allocating nothing. Pass them to `Renderer.renderCamera`.
    ///
    /// # Errors
    /// Returns an error for a non-finite time or a buffer that is not three
    /// values long.
    #[wasm_bindgen(js_name = sampleInto)]
    pub fn sample_into(
        &self,
        seconds: f64,
        position: &js_sys::Float32Array,
        target: &js_sys::Float32Array,
        up: &js_sys::Float32Array,
    ) -> Result<(), JsError> {
        // Validate every buffer before writing any of them.
        vector3(position, "position")?;
        vector3(target, "target")?;
        vector3(up, "up")?;
        let camera = self
            .inner
            .sample(seconds)
            .ok_or_else(|| JsError::new("camera path time must be finite"))?;
        position.copy_from(&camera.eye.to_array());
        target.copy_from(&camera.target.to_array());
        up.copy_from(&camera.up.to_array());
        Ok(())
    }

    /// First and last keyframe time, in playback seconds.
    #[must_use]
    pub fn range(&self) -> Vec<f64> {
        self.inner.range().to_vec()
    }

    /// Number of keyframes.
    #[must_use]
    #[wasm_bindgen(getter, js_name = keyframeCount)]
    pub fn keyframe_count(&self) -> usize {
        self.inner.keyframes().len()
    }
}

#[wasm_bindgen(js_name = CameraBookmark)]
#[derive(Clone, Debug)]
/// A named camera at one playback timestamp.
pub struct WebCameraBookmark {
    inner: CameraBookmark,
}

#[wasm_bindgen(js_class = CameraBookmark)]
impl WebCameraBookmark {
    /// Captures a camera under a non-empty label at a finite playback time.
    ///
    /// # Errors
    /// Returns an error for an empty label or non-finite time.
    #[wasm_bindgen(constructor)]
    pub fn new(
        label: &str,
        seconds: f64,
        camera: &WebCamera,
    ) -> Result<WebCameraBookmark, JsError> {
        CameraBookmark::new(label, seconds, camera.inner)
            .map(|inner| Self { inner })
            .map_err(javascript_error)
    }

    /// Human-readable label.
    #[must_use]
    #[wasm_bindgen(getter)]
    pub fn label(&self) -> String {
        self.inner.label().to_owned()
    }

    /// Captured playback timestamp, in seconds.
    #[must_use]
    #[wasm_bindgen(getter)]
    pub fn seconds(&self) -> f64 {
        self.inner.time_seconds()
    }

    /// The captured camera.
    #[must_use]
    #[wasm_bindgen(getter)]
    pub fn camera(&self) -> WebCamera {
        WebCamera {
            inner: self.inner.camera(),
        }
    }

    /// Encodes the bookmark as JSON metadata.
    ///
    /// # Errors
    /// Returns an error if encoding fails.
    #[wasm_bindgen(js_name = toJSON)]
    pub fn to_json(&self) -> Result<String, JsError> {
        self.inner.to_json().map_err(javascript_error)
    }

    /// Decodes and validates bookmark JSON.
    ///
    /// # Errors
    /// Returns an error for malformed JSON or invalid camera state.
    #[wasm_bindgen(js_name = fromJSON)]
    pub fn from_json(source: &str) -> Result<WebCameraBookmark, JsError> {
        CameraBookmark::from_json(source)
            .map(|inner| Self { inner })
            .map_err(javascript_error)
    }
}
