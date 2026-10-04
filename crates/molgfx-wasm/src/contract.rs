//! JavaScript adapters over the exact Rust/Python scene wire contract.

use num_traits::ToPrimitive as _;
use std::collections::BTreeMap;
use wasm_bindgen::prelude::*;
mod segmentation;
#[cfg(test)]
#[path = "contract/volume_tests.rs"]
mod volume_tests;

pub(crate) fn javascript_error(error: impl std::fmt::Display) -> JsError {
    JsError::new(&error.to_string())
}

#[wasm_bindgen(js_name = SceneSpec)]
#[derive(Clone, Debug)]
/// Renderer-independent JSON scene state, including planes and unit-cell guides.
pub struct WebSceneSpec {
    inner: molgfx::SceneSpec,
}

#[wasm_bindgen(js_class = SceneSpec)]
impl WebSceneSpec {
    /// Parses the canonical scene JSON shared with Rust and Python.
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
    volume_bindings: BTreeMap<Box<str>, molgfx::VolumeBinding>,
    segmentation_bindings: BTreeMap<Box<str>, molgfx::SegmentationBinding>,
    pub(crate) resolved: Option<molgfx::Scene>,
}

#[wasm_bindgen(js_class = Scene)]
impl WebScene {
    /// Creates a resolved scene with no molecular sources.
    #[must_use]
    pub fn empty() -> WebScene {
        let resolved = molgfx::Scene::empty();
        Self {
            spec: resolved.to_spec(),
            structures: BTreeMap::new(),
            volume_bindings: BTreeMap::new(),
            segmentation_bindings: BTreeMap::new(),
            resolved: Some(resolved),
        }
    }

    /// Binds scalar values to an authored affine volume.
    ///
    /// # Errors
    /// Returns an error for an unresolved scene, unknown identity or invalid grid.
    #[wasm_bindgen(js_name = bindVolume)]
    pub fn bind_volume(&mut self, identity: u64, values: Vec<f32>) -> Result<(), JsError> {
        let scene = self
            .resolved
            .as_mut()
            .ok_or_else(|| JsError::new("resolve the scene before binding a volume"))?;
        let spec = scene
            .spec()
            .volumes
            .get(&molgfx::VolumeId::new(identity))
            .ok_or_else(|| JsError::new("unknown volume identity"))?;
        let content_hash = spec.source.content_hash.clone();
        let binding = molgfx::VolumeBinding::new(
            spec.source.clone(),
            spec.dimensions,
            std::sync::Arc::from(values),
        )
        .affine(spec.voxel_to_world);
        scene
            .bind_volume(binding.clone())
            .map_err(javascript_error)?;
        let _ = self.volume_bindings.insert(content_hash, binding);
        self.spec = scene.to_spec();
        Ok(())
    }

    /// Reports overlay descriptors with no matching runtime data.
    ///
    /// # Errors
    /// Returns an error when the scene is unresolved or JSON encoding fails.
    #[wasm_bindgen(js_name = unresolvedOverlays)]
    pub fn unresolved_overlays(&self) -> Result<String, JsError> {
        serde_json::to_string(
            &self
                .resolved
                .as_ref()
                .ok_or_else(|| JsError::new("scene is unresolved"))?
                .unresolved_overlays(),
        )
        .map_err(javascript_error)
    }
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
            volume_bindings: BTreeMap::new(),
            segmentation_bindings: BTreeMap::new(),
            resolved: None,
        })
    }

    /// Parses local molecular bytes and starts a resolved one-structure scene.
    ///
    /// The bytes never leave the browser. The parser recognizes the formats
    /// compiled into the browser runtime, including mmCIF, `BinaryCIF`, and PDB.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error when the data cannot be parsed or adapted
    /// into a `MolGFX` scene.
    #[wasm_bindgen(js_name = fromStructureBytes)]
    pub fn from_structure_bytes(bytes: Vec<u8>, name: Option<String>) -> Result<WebScene, JsError> {
        let name = name
            .into_iter()
            .fold("structure".to_owned(), |_, value| value);
        let (structure, _) =
            molframe::read_bytes(bytes, Some(&name), &molframe::ReadOptions::new())
                .map_err(javascript_error)?;
        let resolved = molgfx::Scene::from_structure(&structure).map_err(javascript_error)?;
        let spec = resolved.to_spec();
        let mut structures = BTreeMap::new();
        let _ = structures.insert(molgfx::StructureId::new(1), structure);
        Ok(Self {
            spec,
            structures,
            volume_bindings: BTreeMap::new(),
            segmentation_bindings: BTreeMap::new(),
            resolved: Some(resolved),
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

    /// Updates the resident trajectory sample without recording a command.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error when the scene is unresolved or the trajectory
    /// sample is invalid.
    pub fn set_trajectory_time(
        &mut self,
        structure: u64,
        sample_seconds: f32,
    ) -> Result<(), JsError> {
        self.resolved
            .as_mut()
            .ok_or_else(|| JsError::new("scene must be resolved before trajectory updates"))?
            .set_trajectory_time(molgfx::StructureId::new(structure), sample_seconds)
            .map_err(javascript_error)
    }
    /// Validates all bindings and creates the physical renderer scene.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error for missing or content-mismatched sources.
    pub fn resolve(&mut self) -> Result<(), JsError> {
        let mut resolved = molgfx::Scene::from_spec(self.spec.clone(), self.structures.clone())
            .map_err(javascript_error)?;
        for binding in self.volume_bindings.values() {
            resolved
                .bind_volume(binding.clone())
                .map_err(javascript_error)?;
        }
        for binding in self.segmentation_bindings.values() {
            resolved
                .bind_segmentation(binding.clone())
                .map_err(javascript_error)?;
        }
        self.spec = resolved.to_spec();
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
        self.volume_bindings.retain(|hash, _| {
            candidate_spec
                .volumes
                .values()
                .any(|volume| volume.source.content_hash == *hash)
        });
        self.segmentation_bindings.retain(|hash, _| {
            candidate_spec
                .segmentations
                .values()
                .any(|segmentation| segmentation.source.content_hash == *hash)
        });
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
    /// Returns residue metadata for a resolved structure as JSON.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error when the scene is unresolved, the structure is
    /// unknown, or the metadata cannot be encoded.
    #[wasm_bindgen(js_name = residueMetadataJSON)]
    pub fn residue_metadata_json(&self, structure: u64) -> Result<String, JsError> {
        self.resolved
            .as_ref()
            .ok_or_else(|| JsError::new("scene must be resolved before residue metadata"))?
            .residue_metadata(molgfx::StructureId::new(structure))
            .and_then(|metadata| {
                serde_json::to_string(&metadata)
                    .map_err(|error| molgfx::Error::InvalidSpec(error.to_string()))
            })
            .map_err(javascript_error)
    }

    /// The inferred or explicitly authored camera for a canvas.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error while the scene remains unresolved.
    pub fn camera(&self, width: u32, height: u32) -> Result<crate::camera::WebCamera, JsError> {
        let aspect = Self::aspect(width, height)?;
        let resolved = self
            .resolved
            .as_ref()
            .ok_or_else(|| JsError::new("scene must be resolved before framing"))?;
        Ok(crate::camera::WebCamera {
            inner: resolved.framing_camera(aspect),
        })
    }

    /// A camera that frames the atoms a selection picks on a canvas, leaving
    /// the scene as it is.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error while the scene is unresolved, when the query
    /// is invalid, or when it picks no atom.
    pub fn frame(
        &self,
        selection: &str,
        width: u32,
        height: u32,
    ) -> Result<crate::camera::WebCamera, JsError> {
        let aspect = Self::aspect(width, height)?;
        self.resolved
            .as_ref()
            .ok_or_else(|| JsError::new("scene must be resolved before framing"))?
            .frame(selection, aspect)
            .map(|inner| crate::camera::WebCamera { inner })
            .map_err(javascript_error)
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
            "distance": distance(camera.eye.to_array(), camera.target.to_array()),
        }))
        .map_err(javascript_error)
    }
}

impl WebScene {
    /// Width over height of a canvas, from its pixel size.
    fn aspect(width: u32, height: u32) -> Result<f32, JsError> {
        let width = width
            .max(1)
            .to_f32()
            .ok_or_else(|| JsError::new("canvas width cannot be represented"))?;
        let height = height
            .max(1)
            .to_f32()
            .ok_or_else(|| JsError::new("canvas height cannot be represented"))?;
        Ok(width / height)
    }
}

pub(crate) fn vector3(value: &js_sys::Float32Array, name: &str) -> Result<[f32; 3], JsError> {
    if value.length() != 3 {
        return Err(JsError::new(&format!("{name} must have three values")));
    }
    Ok([value.get_index(0), value.get_index(1), value.get_index(2)])
}

pub(crate) fn distance(from: [f32; 3], to: [f32; 3]) -> f32 {
    let x = to[0] - from[0];
    let y = to[1] - from[1];
    let z = to[2] - from[2];
    z.mul_add(z, x.mul_add(x, y * y)).sqrt()
}
