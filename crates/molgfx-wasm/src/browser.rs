//! JavaScript marshalling only; rendering and selection remain in Rust.

pub use crate::browser_camera::WebCamera;
pub use crate::browser_controls::{
    WebCameraController, WebCameraControllerKind, WebNavigationKey, WebPointerButton,
};
pub use crate::browser_engine::WebEngine;
pub use crate::browser_frame::{WebFrameReport, WebFrameTiming};
pub use crate::browser_presentation::{
    WebClipCap, WebClipPlane, WebClipSet, WebMaterial, WebMaterialModel,
};
pub use crate::browser_types::{
    WebCapabilities, WebEngineConfig, WebLifecycleState, WebPick, WebPickKind, WebPowerPreference,
    WebProfile, WebRenderMode, WebResolutionPolicy,
};
use molgfx::{Camera, ColorScheme, Representation, RepresentationConfig, Rgba8, Scene};
use wasm_bindgen::prelude::*;

use crate::browser_secondary::parse_structure;

/// Every molecular representation accepted by the portable renderer.
#[wasm_bindgen]
#[derive(Clone, Copy, Debug)]
pub enum WebRepresentationKind {
    /// Van der Waals spheres.
    Spacefill,
    /// Atom spheres and bond capsules.
    BallAndStick,
    /// Round pixel-stable bonds.
    Lines,
    /// Uniform atom and bond radii.
    Licorice,
    /// Secondary-structure ribbon.
    Cartoon,
    /// Backbone trace.
    Trace,
    /// Smooth backbone tube.
    Tube,
    /// Molecular surface.
    Surface,
    /// Pixel-stable dots.
    Points,
    /// One sphere per residue.
    Beads,
    /// Solid secondary-structure bodies.
    Rocket,
    /// Glycan twist chain.
    Twister,
    /// Glycan paper chain.
    PaperChain,
}

/// A target-independent declarative representation recipe for JavaScript.
#[wasm_bindgen]
#[derive(Clone, Debug)]
pub struct WebRepresentation {
    pub(crate) inner: RepresentationConfig,
}

#[wasm_bindgen]
impl WebRepresentation {
    /// Creates any portable molecular recipe without a JS-side switch table.
    #[must_use]
    pub fn of(kind: WebRepresentationKind) -> Self {
        let inner = match kind {
            WebRepresentationKind::Spacefill => Representation::spacefill(),
            WebRepresentationKind::BallAndStick => Representation::ball_and_stick(),
            WebRepresentationKind::Lines => Representation::lines(),
            WebRepresentationKind::Licorice => Representation::licorice(),
            WebRepresentationKind::Cartoon => Representation::cartoon(),
            WebRepresentationKind::Trace => Representation::trace(),
            WebRepresentationKind::Tube => Representation::tube(),
            WebRepresentationKind::Surface => Representation::surface(),
            WebRepresentationKind::Points => Representation::points(),
            WebRepresentationKind::Beads => Representation::beads(),
            WebRepresentationKind::Rocket => Representation::rocket(),
            WebRepresentationKind::Twister => Representation::twister(),
            WebRepresentationKind::PaperChain => Representation::paper_chain(),
        };
        Self { inner }
    }

    /// Molecular surface recipe.
    #[must_use]
    pub fn surface() -> Self {
        Self {
            inner: Representation::surface(),
        }
    }

    /// Licorice atom-and-bond recipe.
    #[must_use]
    pub fn licorice() -> Self {
        Self {
            inner: Representation::licorice(),
        }
    }

    /// Cartoon ribbon recipe.
    #[must_use]
    pub fn cartoon() -> Self {
        Self {
            inner: Representation::cartoon(),
        }
    }

    /// Space-filling atom recipe.
    #[must_use]
    pub fn spacefill() -> Self {
        Self {
            inner: Representation::spacefill(),
        }
    }

    /// Returns a recipe with one uniform RGB color.
    #[must_use]
    pub fn color(self, red: u8, green: u8, blue: u8) -> Self {
        Self {
            inner: self
                .inner
                .color(ColorScheme::Uniform(Rgba8::opaque(red, green, blue))),
        }
    }

    /// Returns a recipe coloured by molecular chain.
    #[must_use]
    #[wasm_bindgen(js_name = colorByChain)]
    pub fn color_by_chain(self) -> Self {
        Self {
            inner: self.inner.color(ColorScheme::ByChain),
        }
    }

    /// Returns a recipe coloured by deposited secondary-structure class.
    #[must_use]
    #[wasm_bindgen(js_name = colorBySecondaryStructure)]
    pub fn color_by_secondary_structure(self) -> Self {
        Self {
            inner: self.inner.color(ColorScheme::BySecondaryStructure),
        }
    }

    /// Returns a recipe coloured by the conventional element palette.
    #[must_use]
    #[wasm_bindgen(js_name = colorByElement)]
    pub fn color_by_element(self) -> Self {
        Self {
            inner: self.inner.color(ColorScheme::ByElement),
        }
    }

    /// Returns a recipe coloured by stable source residue row.
    #[must_use]
    #[wasm_bindgen(js_name = colorByResidue)]
    pub fn color_by_residue(self) -> Self {
        Self {
            inner: self.inner.color(ColorScheme::ByResidue),
        }
    }

    /// Applies one typed material to the whole batched representation.
    #[must_use]
    pub fn material(self, material: &WebMaterial) -> Self {
        Self {
            inner: self.inner.material(material.inner),
        }
    }

    /// Applies fixed-capacity world-space clipping without rebuilding source geometry.
    #[must_use]
    pub fn clipping(self, clipping: &WebClipSet) -> Self {
        Self {
            inner: self.inner.clipping(clipping.inner),
        }
    }

    /// Returns a recipe with scaled atomic radii.
    #[must_use]
    #[wasm_bindgen(js_name = radiusScale)]
    pub fn radius_scale(self, scale: f32) -> Self {
        Self {
            inner: self.inner.radius_scale(scale),
        }
    }

    /// Returns a recipe with a bond capsule radius in angstrom.
    #[must_use]
    #[wasm_bindgen(js_name = bondRadius)]
    pub fn bond_radius(self, radius: f32) -> Self {
        Self {
            inner: self.inner.bond_radius(radius),
        }
    }
}

/// A browser-owned scene retaining its parsed structural source.
#[wasm_bindgen]
#[derive(Debug)]
pub struct WebScene {
    sources: Vec<pdbiox::Structure>,
    pub(crate) inner: Scene,
}

#[wasm_bindgen]
impl WebScene {
    /// Creates an empty scene for generic points, instances and relations.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            sources: Vec::new(),
            inner: Scene::new(),
        }
    }

    /// Copies caller-supplied bytes once, parses through `pdbiox`, and creates a scene.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error when parsing or scene construction fails.
    #[wasm_bindgen(js_name = copyFromBytes)]
    pub fn copy_from_bytes(bytes: Vec<u8>, name: Option<String>) -> Result<WebScene, JsValue> {
        let parsed = parse_structure(bytes, name)
            .map_err(|diagnostics| js_error("structure parse failed", &diagnostics))?;
        let mut inner = Scene::new();
        let handle = inner
            .add_structure(&parsed.structure)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        if !parsed.secondary_structure.is_empty() {
            inner
                .apply_secondary_structure(handle, &parsed.secondary_structure)
                .map_err(|error| JsValue::from_str(&error.to_string()))?;
        }
        Ok(Self {
            sources: vec![parsed.structure],
            inner,
        })
    }

    /// Parses and places another candidate without crossing a per-atom JS API.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error when parsing or scene construction fails.
    #[wasm_bindgen(js_name = addStructure)]
    pub fn add_structure(&mut self, bytes: Vec<u8>, name: Option<String>) -> Result<u32, JsValue> {
        let parsed = parse_structure(bytes, name)
            .map_err(|diagnostics| js_error("structure parse failed", &diagnostics))?;
        let handle = self
            .inner
            .add_structure(&parsed.structure)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        if !parsed.secondary_structure.is_empty() {
            self.inner
                .apply_secondary_structure(handle, &parsed.secondary_structure)
                .map_err(|error| JsValue::from_str(&error.to_string()))?;
        }
        self.sources.push(parsed.structure);
        Ok(handle.row())
    }

    /// Compiles a query and installs one GPU-batched representation.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error for invalid selections or representations.
    pub fn represent(
        &mut self,
        selection: &str,
        representation: WebRepresentation,
    ) -> Result<(), JsValue> {
        self.inner
            .represent(selection, representation.inner)
            .map(|_| ())
            .map_err(|error| JsValue::from_str(&error.to_string()))
    }

    /// Camera framing the current scene bounds.
    #[must_use]
    pub fn camera(&self) -> WebCamera {
        WebCamera {
            inner: Camera::framing_aabb(&self.inner.world_aabb(), 1.0),
        }
    }
}

fn js_error(prefix: &str, diagnostics: &[pdbiox::Diagnostic]) -> JsValue {
    JsValue::from_str(&format!("{prefix}: {diagnostics:?}"))
}
