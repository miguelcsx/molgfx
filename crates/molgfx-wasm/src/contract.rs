//! JavaScript adapters over the exact Rust/Python scene wire contract.

use num_traits::ToPrimitive as _;
use std::collections::BTreeMap;
use wasm_bindgen::prelude::*;

pub(crate) fn javascript_error(error: impl std::fmt::Display) -> JsError {
    JsError::new(&error.to_string())
}

#[wasm_bindgen(js_name = SceneSpec)]
#[derive(Clone, Debug)]
/// Versioned renderer-independent scene contract for JavaScript.
pub struct WebSceneSpec {
    inner: molgfx::SceneSpec,
}

#[wasm_bindgen(js_class = SceneSpec)]
impl WebSceneSpec {
    /// Parses one versioned canonical scene specification.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error for malformed or unsupported input.
    #[wasm_bindgen(constructor)]
    pub fn new(source: &str) -> Result<WebSceneSpec, JsError> {
        molgfx::SceneSpec::from_json(source)
            .map(|inner| Self { inner })
            .map_err(|error| JsError::new(&error.to_string()))
    }

    /// Deterministically serializes this specification.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error if serialization fails.
    #[wasm_bindgen(js_name = toJSON)]
    pub fn to_json(&self) -> Result<String, JsError> {
        self.inner
            .to_json()
            .map_err(|error| JsError::new(&error.to_string()))
    }

    /// Current semantic revision.
    #[must_use]
    #[wasm_bindgen(getter)]
    pub fn revision(&self) -> u64 {
        self.inner.revision
    }

    /// Applies one revision-checked patch atomically.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error on conflict or invalid resulting state.
    pub fn apply(&mut self, patch: &WebScenePatch) -> Result<(), JsError> {
        let candidate = self
            .inner
            .patched(&patch.inner)
            .map_err(|error| JsError::new(&error.to_string()))?;
        self.inner = candidate;
        Ok(())
    }
}

#[wasm_bindgen(js_name = ScenePatch)]
#[derive(Clone, Debug)]
/// Atomic incremental scene update for JavaScript.
pub struct WebScenePatch {
    inner: molgfx::ScenePatch,
}

#[wasm_bindgen(js_class = ScenePatch)]
impl WebScenePatch {
    /// Parses one incremental scene patch.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error for malformed input.
    #[wasm_bindgen(constructor)]
    pub fn new(source: &str) -> Result<WebScenePatch, JsError> {
        serde_json::from_str(source)
            .map(|inner| Self { inner })
            .map_err(|error| JsError::new(&error.to_string()))
    }

    /// Required base revision.
    #[must_use]
    #[wasm_bindgen(getter, js_name = baseRevision)]
    pub fn base_revision(&self) -> u64 {
        self.inner.base_revision
    }

    /// Deterministically serializes this patch.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error if serialization fails.
    #[wasm_bindgen(js_name = toJSON)]
    pub fn to_json(&self) -> Result<String, JsError> {
        serde_json::to_string(&self.inner).map_err(|error| JsError::new(&error.to_string()))
    }
}

#[wasm_bindgen(js_name = Scene)]
#[derive(Debug)]
/// Resolved browser scene retaining parsed molecular storage by shared ownership.
pub struct WebScene {
    pub(crate) spec: molgfx::SceneSpec,
    structures: BTreeMap<molgfx::StructureId, molframe::Structure>,
    pub(crate) resolved: Option<molgfx::Scene>,
}

#[wasm_bindgen(js_class = Scene)]
impl WebScene {
    /// Creates an unresolved scene from the portable semantic contract.
    ///
    /// Bind every declared molecular source, then call `resolve` before rendering.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error for malformed or unsupported input.
    #[wasm_bindgen(constructor)]
    pub fn new(source: &str) -> Result<WebScene, JsError> {
        let spec = molgfx::SceneSpec::from_json(source).map_err(javascript_error)?;
        Ok(Self {
            spec,
            structures: BTreeMap::new(),
            resolved: None,
        })
    }

    /// Parses and binds one molecular source to its semantic identity.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error for an undeclared identity or malformed data.
    #[wasm_bindgen(js_name = bindStructure)]
    pub fn bind_structure(
        &mut self,
        identity: u64,
        bytes: Vec<u8>,
        name: Option<String>,
    ) -> Result<(), JsError> {
        let identity = molgfx::StructureId::new(identity);
        if !self.spec.structures.contains_key(&identity) {
            return Err(JsError::new(
                "structure identity is not declared by SceneSpec",
            ));
        }
        let name = name
            .into_iter()
            .fold("structure".to_owned(), |_, value| value);
        let (structure, _) =
            molframe::read_bytes(bytes, Some(&name), &molframe::ReadOptions::new())
                .map_err(javascript_error)?;
        let _ = self.structures.insert(identity, structure);
        self.resolved = None;
        Ok(())
    }

    /// Validates all bindings and creates the physical renderer scene.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error for missing or content-mismatched sources.
    pub fn resolve(&mut self) -> Result<(), JsError> {
        let resolved = molgfx::Scene::from_spec(self.spec.clone(), self.structures.clone())
            .map_err(javascript_error)?;
        self.resolved = Some(resolved);
        Ok(())
    }

    /// Applies an incremental patch atomically through the shared runtime scene.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error on conflicts or invalid resulting state.
    pub fn apply(&mut self, patch: &WebScenePatch) -> Result<(), JsError> {
        let candidate_spec = self.spec.patched(&patch.inner).map_err(javascript_error)?;
        if let Some(resolved) = &mut self.resolved {
            resolved.apply(&patch.inner).map_err(javascript_error)?;
        }
        self.spec = candidate_spec;
        Ok(())
    }

    /// Current semantic revision.
    #[must_use]
    #[wasm_bindgen(getter)]
    pub fn revision(&self) -> u64 {
        self.spec.revision
    }

    /// Whether all molecular sources have been resolved successfully.
    #[must_use]
    #[wasm_bindgen(getter, js_name = isReady)]
    pub fn is_ready(&self) -> bool {
        self.resolved.is_some()
    }

    /// Returns the inferred or explicitly authored camera for a canvas aspect.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error while the scene remains unresolved.
    #[wasm_bindgen(js_name = cameraJSON)]
    pub fn camera_json(&self, width: u32, height: u32) -> Result<String, JsError> {
        let resolved = self
            .resolved
            .as_ref()
            .ok_or_else(|| JsError::new("scene must be resolved before framing"))?;
        let width = width
            .max(1)
            .to_f32()
            .ok_or_else(|| JsError::new("canvas width cannot be represented"))?;
        let height = height
            .max(1)
            .to_f32()
            .ok_or_else(|| JsError::new("canvas height cannot be represented"))?;
        let camera = resolved.framing_camera(width / height);
        serde_json::to_string(&serde_json::json!({
            "position": camera.eye.to_array(),
            "target": camera.target.to_array(),
            "up": camera.up.to_array(),
        }))
        .map_err(javascript_error)
    }
}

#[wasm_bindgen(js_name = Renderer)]
#[derive(Debug)]
/// Browser WebGPU renderer attached directly to a canvas.
pub struct WebRenderer {
    inner: molgfx::Renderer,
    width: u32,
    height: u32,
}

#[wasm_bindgen(js_class = Renderer)]
impl WebRenderer {
    /// Opens WebGPU asynchronously without a low-level configuration object.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error when the browser has no compatible adapter.
    #[wasm_bindgen(js_name = create)]
    pub async fn create(canvas: web_sys::HtmlCanvasElement) -> Result<WebRenderer, JsError> {
        let width = canvas.width().max(1);
        let height = canvas.height().max(1);
        let mut inner = molgfx::Renderer::for_canvas(canvas, molgfx::RenderProfile::default())
            .await
            .map_err(javascript_error)?;
        inner.resize((width, height));
        Ok(Self {
            inner,
            width,
            height,
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
    pub fn render(&mut self, scene: &WebScene) -> Result<(), JsError> {
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
    ) -> Result<(), JsError> {
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
            .map_err(javascript_error)
    }

    /// Resolves one canvas pixel through the renderer's single packed map.
    ///
    /// The returned JSON contains stable dataset/chunk/row provenance rather
    /// than a physical GPU token. `None` denotes the background.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error if readback or semantic resolution fails.
    pub async fn pick(&mut self, x: u32, y: u32) -> Result<Option<String>, JsError> {
        self.inner
            .pick_async(x, y)
            .await
            .map_err(javascript_error)?
            .map(|pick| pick.to_json().map_err(javascript_error))
            .transpose()
    }
}

fn vector3(value: &js_sys::Float32Array, name: &str) -> Result<[f32; 3], JsError> {
    if value.length() != 3 {
        return Err(JsError::new(&format!("{name} must have three values")));
    }
    Ok([value.get_index(0), value.get_index(1), value.get_index(2)])
}

fn distance(from: [f32; 3], to: [f32; 3]) -> f32 {
    let x = to[0] - from[0];
    let y = to[1] - from[1];
    let z = to[2] - from[2];
    z.mul_add(z, x.mul_add(x, y * y)).sqrt()
}
