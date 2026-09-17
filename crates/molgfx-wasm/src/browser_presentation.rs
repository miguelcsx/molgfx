//! Thin material and world-space clipping recipes for browser representations.

use molgfx::{ClipCap, ClipPlane, ClipSet, Material, MaterialModel, Vec3};
use wasm_bindgen::prelude::*;

/// Tagged lighting response selected per representation.
#[wasm_bindgen]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WebMaterialModel {
    /// Restrained dielectric response for scientific inspection.
    Molecular,
    /// Energy-conserving metalness workflow.
    Principled,
    /// Tangent-aligned polymer-ribbon response.
    AnisotropicRibbon,
    /// Bounded screen-space diffusion cue.
    Diffusion,
}

/// Immutable surface-response recipe delegated to the shared renderer.
#[wasm_bindgen]
#[derive(Clone, Copy, Debug)]
pub struct WebMaterial {
    pub(crate) inner: Material,
}

#[wasm_bindgen]
#[allow(clippy::must_use_candidate)]
impl WebMaterial {
    /// Scientific molecular material defaults.
    pub fn molecular() -> Self {
        Self {
            inner: Material::default(),
        }
    }

    /// Explicit principled material with renderer-sanitized metalness.
    pub fn principled(metallic: f32) -> Self {
        Self {
            inner: Material::principled(metallic),
        }
    }

    /// Tangent-aligned response for polymer ribbons.
    #[wasm_bindgen(js_name = anisotropicRibbon)]
    pub fn anisotropic_ribbon(strength: f32) -> Self {
        Self {
            inner: Material::anisotropic_ribbon(strength),
        }
    }

    /// Bounded diffusion presentation.
    pub fn diffusion(strength: f32) -> Self {
        Self {
            inner: Material::diffusion(strength),
        }
    }

    /// Returns a copy with the requested opacity.
    #[wasm_bindgen(js_name = withOpacity)]
    pub fn with_opacity(mut self, opacity: f32) -> Self {
        self.inner.opacity = opacity;
        self
    }

    /// Returns a copy with the requested perceptual roughness.
    #[wasm_bindgen(js_name = withRoughness)]
    pub fn with_roughness(mut self, roughness: f32) -> Self {
        self.inner.roughness = roughness;
        self
    }

    /// Returns a copy with the requested specular strength.
    #[wasm_bindgen(js_name = withSpecular)]
    pub fn with_specular(mut self, specular: f32) -> Self {
        self.inner.specular = specular;
        self
    }

    /// Opacity supplied to the shared material contract.
    #[wasm_bindgen(getter)]
    pub fn opacity(&self) -> f32 {
        self.inner.opacity
    }

    /// Sanitized roughness consumed by the renderer.
    #[wasm_bindgen(getter)]
    pub fn roughness(&self) -> f32 {
        self.inner.perceptual_roughness()
    }

    /// Sanitized specular strength consumed by the renderer.
    #[wasm_bindgen(getter)]
    pub fn specular(&self) -> f32 {
        self.inner.specular_strength()
    }

    /// Tagged response model.
    #[wasm_bindgen(getter)]
    pub fn model(&self) -> WebMaterialModel {
        match self.inner.model {
            MaterialModel::Molecular => WebMaterialModel::Molecular,
            MaterialModel::Principled { .. } => WebMaterialModel::Principled,
            MaterialModel::AnisotropicRibbon { .. } => WebMaterialModel::AnisotropicRibbon,
            MaterialModel::Diffusion { .. } => WebMaterialModel::Diffusion,
        }
    }

    /// Sanitized model-specific parameter, or zero for molecular response.
    #[wasm_bindgen(getter, js_name = modelStrength)]
    pub fn model_strength(&self) -> f32 {
        match self.inner.model {
            MaterialModel::Molecular => 0.0,
            MaterialModel::Principled { .. } => self.inner.metallic(),
            MaterialModel::AnisotropicRibbon { .. } => self.inner.anisotropy(),
            MaterialModel::Diffusion { .. } => self.inner.diffusion_strength(),
        }
    }
}

/// Treatment of molecular interiors exposed by clipping.
#[wasm_bindgen]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WebClipCap {
    /// Leave the cut open.
    Open,
    /// Draw a flat depth-correct cross-section.
    Solid,
}

/// Validated world-space half-space.
#[wasm_bindgen]
#[derive(Clone, Copy, Debug)]
pub struct WebClipPlane {
    inner: ClipPlane,
}

#[wasm_bindgen]
#[allow(clippy::must_use_candidate, clippy::too_many_arguments)]
impl WebClipPlane {
    /// Builds a normalized plane through a point.
    ///
    /// # Errors
    ///
    /// Returns a typed error for non-finite coordinates or a zero normal.
    #[wasm_bindgen(js_name = fromPointNormal)]
    pub fn from_point_normal(
        point_x: f32,
        point_y: f32,
        point_z: f32,
        normal_x: f32,
        normal_y: f32,
        normal_z: f32,
    ) -> Result<Self, JsValue> {
        ClipPlane::from_point_normal(
            Vec3::new(point_x, point_y, point_z),
            Vec3::new(normal_x, normal_y, normal_z),
        )
        .map(|inner| Self { inner })
        .map_err(core_error)
    }

    /// Reverses which half-space remains visible.
    pub fn reversed(&self) -> Self {
        Self {
            inner: self.inner.reversed(),
        }
    }

    /// Signed distance delegated to the shared clipping primitive.
    #[wasm_bindgen(js_name = signedDistance)]
    pub fn signed_distance(&self, x: f32, y: f32, z: f32) -> f32 {
        self.inner.signed_distance(Vec3::new(x, y, z))
    }

    /// X component of the normalized plane normal.
    #[wasm_bindgen(getter, js_name = normalX)]
    pub fn normal_x(&self) -> f32 {
        self.inner.normal.x
    }
    /// Y component of the normalized plane normal.
    #[wasm_bindgen(getter, js_name = normalY)]
    pub fn normal_y(&self) -> f32 {
        self.inner.normal.y
    }
    /// Z component of the normalized plane normal.
    #[wasm_bindgen(getter, js_name = normalZ)]
    pub fn normal_z(&self) -> f32 {
        self.inner.normal.z
    }
    /// Signed plane offset in world-space units.
    #[wasm_bindgen(getter)]
    pub fn offset(&self) -> f32 {
        self.inner.offset
    }
}

/// Validated fixed-capacity clipping recipe.
#[wasm_bindgen]
#[derive(Clone, Copy, Debug, Default)]
pub struct WebClipSet {
    pub(crate) inner: ClipSet,
}

#[wasm_bindgen]
#[allow(clippy::must_use_candidate)]
impl WebClipSet {
    /// Creates an empty clipping set.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates two opposing planes retaining a slab.
    ///
    /// # Errors
    ///
    /// Returns a typed error for invalid center, normal or thickness.
    #[allow(clippy::too_many_arguments)]
    pub fn slab(
        center_x: f32,
        center_y: f32,
        center_z: f32,
        normal_x: f32,
        normal_y: f32,
        normal_z: f32,
        thickness: f32,
    ) -> Result<Self, JsValue> {
        ClipSet::slab(
            Vec3::new(center_x, center_y, center_z),
            Vec3::new(normal_x, normal_y, normal_z),
            thickness,
        )
        .map(|inner| Self { inner })
        .map_err(core_error)
    }

    /// Appends one plane while preserving the four-plane portable limit.
    ///
    /// # Errors
    ///
    /// Returns a typed error when a fifth plane is appended.
    #[wasm_bindgen(js_name = addPlane)]
    pub fn add_plane(&mut self, plane: &WebClipPlane) -> Result<(), JsValue> {
        let mut planes = self.inner.planes().to_vec();
        planes.push(plane.inner);
        self.inner = ClipSet::new(&planes)
            .map_err(core_error)?
            .with_cap(self.inner.cap());
        Ok(())
    }

    /// Selects open or solid clipped interiors.
    #[wasm_bindgen(js_name = setCap)]
    pub fn set_cap(&mut self, cap: WebClipCap) {
        self.inner = self.inner.with_cap(match cap {
            WebClipCap::Open => ClipCap::Open,
            WebClipCap::Solid => ClipCap::Solid,
        });
    }

    /// Active plane count.
    #[wasm_bindgen(getter, js_name = planeCount)]
    pub fn plane_count(&self) -> u32 {
        match u32::try_from(self.inner.planes().len()) {
            Ok(value) => value,
            Err(_) => u32::MAX,
        }
    }

    /// Current cap behavior.
    #[wasm_bindgen(getter)]
    pub fn cap(&self) -> WebClipCap {
        match self.inner.cap() {
            ClipCap::Open => WebClipCap::Open,
            ClipCap::Solid => WebClipCap::Solid,
        }
    }

    /// Whether a world-space point survives every plane.
    pub fn contains(&self, x: f32, y: f32, z: f32) -> bool {
        self.inner.contains(Vec3::new(x, y, z))
    }
}

#[allow(clippy::needless_pass_by_value)]
fn core_error(error: molgfx::CoreError) -> JsValue {
    let value = js_sys::Error::new(&error.to_string());
    value.set_name(error.code());
    value.into()
}
