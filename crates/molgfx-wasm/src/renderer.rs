//! Canvas renderer, completed-frame metadata and detached picking.

use crate::contract::{WebScene, distance, javascript_error, vector3};
use num_traits::ToPrimitive as _;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(js_name = Renderer)]
#[derive(Debug)]
/// Browser WebGPU renderer attached directly to a canvas.
pub struct WebRenderer {
    inner: molgfx::Renderer,
    width: u32,
    height: u32,
    last_report: Option<molgfx::FrameReport>,
}

#[wasm_bindgen(js_class = Renderer)]
impl WebRenderer {
    /// Serializes observed settings without allocating during frame rendering.
    ///
    /// # Errors
    /// Returns a JavaScript error if metadata cannot be encoded.
    #[wasm_bindgen(js_name = frameReportJSON)]
    pub fn frame_report_json(&self) -> Result<Option<String>, JsError> {
        self.last_report
            .as_ref()
            .map(|report| serde_json::to_string(report).map_err(javascript_error))
            .transpose()
    }

    /// Opens WebGPU with optional quality and refresh-rate policy.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error when the browser has no compatible adapter.
    #[wasm_bindgen(js_name = create)]
    pub async fn create(
        canvas: web_sys::HtmlCanvasElement,
        quality: Option<String>,
        target_fps: Option<u16>,
    ) -> Result<WebRenderer, JsError> {
        let profile = crate::quality::profile(quality.as_deref(), target_fps)?;
        let width = canvas.width().max(1);
        let height = canvas.height().max(1);
        let mut inner = molgfx::Renderer::for_canvas(canvas, profile)
            .await
            .map_err(javascript_error)?;
        inner.resize((width, height));
        Ok(Self {
            inner,
            width,
            height,
            last_report: None,
        })
    }

    /// Resizes the presentation target.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width.max(1);
        self.height = height.max(1);
        self.inner.resize((self.width, self.height));
    }

    /// Renders directly to the attached canvas with an inferred camera.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error if the scene is unresolved or rendering fails.
    pub fn render(&mut self, scene: &WebScene) -> Result<bool, JsError> {
        let resolved = scene
            .resolved
            .as_ref()
            .ok_or_else(|| JsError::new("scene must be resolved before rendering"))?;
        let width = self
            .width
            .to_f32()
            .ok_or_else(|| JsError::new("canvas width cannot be represented"))?;
        let height = self
            .height
            .to_f32()
            .ok_or_else(|| JsError::new("canvas height cannot be represented"))?;
        let camera = resolved.framing_camera(width / height);
        self.inner
            .present(resolved, &camera)
            .map(|report| {
                self.last_report = Some(report);
                report.needs_another_frame
            })
            .map_err(javascript_error)
    }

    /// Renders with camera state owned by the browser viewer controller.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error for invalid camera geometry or render failure.
    #[wasm_bindgen(js_name = renderCamera)]
    pub fn render_camera(
        &mut self,
        scene: &WebScene,
        position: &js_sys::Float32Array,
        target: &js_sys::Float32Array,
        up: &js_sys::Float32Array,
    ) -> Result<bool, JsError> {
        let resolved = scene
            .resolved
            .as_ref()
            .ok_or_else(|| JsError::new("scene must be resolved before rendering"))?;
        let eye = vector3(position, "camera position")?;
        let target = vector3(target, "camera target")?;
        let up = vector3(up, "camera up vector")?;
        let distance = distance(eye, target);
        let width = self
            .width
            .to_f32()
            .ok_or_else(|| JsError::new("canvas width cannot be represented"))?;
        let height = self
            .height
            .to_f32()
            .ok_or_else(|| JsError::new("canvas height cannot be represented"))?;
        // Near and far track the viewing distance so a molecule stays inside
        // the depth range at every zoom. The facade owns camera validity, so
        // the browser rejects exactly the cameras the native path rejects.
        let camera = molgfx::camera::perspective(
            eye,
            target,
            up,
            45_f32.to_radians(),
            width / height,
            (distance * 0.001).max(0.001),
            (distance * 10.0).max(10.0),
        )
        .map_err(javascript_error)?;
        self.inner
            .present(resolved, &camera)
            .map(|report| {
                self.last_report = Some(report);
                report.needs_another_frame
            })
            .map_err(javascript_error)
    }

    /// Resolves one canvas pixel through the renderer's single packed map and
    /// the live semantic scene.
    ///
    /// Atom JSON contains stable source metadata; non-atom JSON retains the
    /// renderer pick kind and provenance. `None` denotes the background.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error if readback or semantic resolution fails.
    pub async fn pick(
        &mut self,
        scene: &WebScene,
        x: u32,
        y: u32,
    ) -> Result<Option<String>, JsError> {
        let Some(readback) = self.begin_pick(x, y)? else {
            return Ok(None);
        };
        let bytes = readback.resolve().await?;
        self.finish_pick(scene, &readback, &bytes)
    }

    /// Records one pixel pick and detaches its readback.
    ///
    /// The returned handle borrows nothing, so a caller may render while the
    /// readback is awaited. `undefined` means the pixel is outside the target
    /// or nothing is drawable. Hand the readback and its resolved bytes to [`Self::finish_pick`].
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error if the pick copies cannot be recorded.
    #[wasm_bindgen(js_name = beginPick)]
    pub fn begin_pick(&mut self, x: u32, y: u32) -> Result<Option<WebPickReadback>, JsError> {
        let readback = self.inner.begin_pick(x, y).map_err(javascript_error)?;
        Ok(readback.map(|inner| WebPickReadback { inner }))
    }

    /// Resolves a [`Self::begin_pick`] readback against the live scene.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error if the scene is unresolved or the identity
    /// cannot be resolved or encoded.
    #[wasm_bindgen(js_name = finishPick)]
    pub fn finish_pick(
        &self,
        scene: &WebScene,
        readback: &WebPickReadback,
        bytes: &[u8],
    ) -> Result<Option<String>, JsError> {
        let resolved = scene
            .resolved
            .as_ref()
            .ok_or_else(|| JsError::new("scene must be resolved before picking"))?;
        self.inner
            .finish_pick(resolved, &readback.inner, bytes)
            .map_err(javascript_error)?
            .map(|pick| serde_json::to_string(&pick).map_err(javascript_error))
            .transpose()
    }
}

#[wasm_bindgen(js_name = PickReadback)]
#[derive(Debug)]
/// A detached pick readback awaiting one frame's identity bytes.
pub struct WebPickReadback {
    inner: molgfx::PickReadback,
}

#[wasm_bindgen(js_class = PickReadback)]
impl WebPickReadback {
    /// Awaits the packed identity bytes without holding the renderer.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error if readback fails.
    pub async fn resolve(&self) -> Result<Vec<u8>, JsError> {
        self.inner.resolve().await.map_err(javascript_error)
    }
}
