//! JavaScript marshalling only; rendering and selection remain in Rust.

use pdviewx::{
    Camera, ColorScheme, Engine, EngineConfig, FrameOutcome, Representation, RepresentationConfig,
    Rgba8, Scene,
};
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

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
    inner: RepresentationConfig,
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
    inner: Scene,
}

#[wasm_bindgen]
impl WebScene {
    /// Parses caller-supplied structure bytes through `pdbiox` and creates a scene.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error when parsing or scene construction fails.
    #[wasm_bindgen(js_name = fromBytes)]
    pub fn from_bytes(bytes: Vec<u8>, name: Option<String>) -> Result<WebScene, JsValue> {
        let structure = parse_structure(bytes, name)?;
        let inner = Scene::from_structure(&structure)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        Ok(Self {
            sources: vec![structure],
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
        let structure = parse_structure(bytes, name)?;
        let handle = self
            .inner
            .add_structure(&structure)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        self.sources.push(structure);
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

/// Browser camera value passed directly to the native renderer.
#[wasm_bindgen]
#[derive(Clone, Copy, Debug)]
pub struct WebCamera {
    inner: Camera,
}

/// WebGPU engine bound to a host-owned canvas.
#[wasm_bindgen]
#[derive(Debug)]
pub struct WebEngine {
    inner: Engine,
}

#[wasm_bindgen]
impl WebEngine {
    /// Opens WebGPU asynchronously against the supplied canvas.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error when WebGPU is unavailable or setup fails.
    pub async fn create(canvas: HtmlCanvasElement) -> Result<WebEngine, JsValue> {
        let width = canvas.width().max(1);
        let height = canvas.height().max(1);
        let config = EngineConfig {
            width,
            height,
            ..EngineConfig::default()
        };
        let inner = Engine::new_async(&config, Some(canvas))
            .await
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        Ok(Self { inner })
    }

    /// Resizes persistent presentation state; zero dimensions clamp to one.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.inner.resize(width.max(1), height.max(1));
    }

    /// Renders one frame and reports whether it reached presentation.
    ///
    /// # Errors
    ///
    /// Returns a JavaScript error on an unrecoverable GPU failure.
    pub fn render(&mut self, scene: &WebScene, camera: &WebCamera) -> Result<bool, JsValue> {
        self.inner
            .render(&scene.inner, &camera.inner)
            .map(|outcome| outcome == FrameOutcome::Presented)
            .map_err(|error| JsValue::from_str(&error.to_string()))
    }
}

fn js_error(prefix: &str, diagnostics: &[pdbiox::Diagnostic]) -> JsValue {
    JsValue::from_str(&format!("{prefix}: {diagnostics:?}"))
}

fn parse_structure(bytes: Vec<u8>, name: Option<String>) -> Result<pdbiox::Structure, JsValue> {
    let options = pdbiox::ReadOptions::new();
    let name = name.map(String::into_boxed_str);
    pdbiox::read_bytes(bytes, name.as_deref(), &options)
        .map(|parsed| parsed.0)
        .map_err(|diagnostics| js_error("structure parse failed", &diagnostics))
}
