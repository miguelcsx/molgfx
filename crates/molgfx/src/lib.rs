//! Host-neutral facade for the public engine surface.
//!
//! Callers import from here only. The crate is a list of re-exports and the
//! feature flags that decide which subsystems get linked; it holds no rendering
//! or scientific logic.
//!
//! The root carries the curated names an ordinary program wants — the scene and
//! the engine that drives it, representations and materials, camera and image,
//! the typed errors. Each subsystem is also published under its own name —
//! `molgfx::core`, `molgfx::render` — so a name lives in exactly one place and
//! a kernel, request record or render-graph type a program wants is one
//! namespace away.
//!
//! Math identities are project-owned and re-exported from [`math`]; glam stays
//! a private implementation detail. Backends are chosen by capability behind
//! [`render::Engine`] and never named.
//!
//! The structural model is re-exported as [`molframe`] because public
//! signatures here name its types — a chunk payload carries a `StructureChunk`,
//! a selection carries a `BitVec` — and a caller restricted to this crate has to
//! be able to say what it is holding.
//!
//! # Drawing a structure
//!
//! ```no_run
//! use molgfx::{
//!     Camera, Engine, EngineConfig, ImageConfig, Representation, RepresentationKind, Scene, Select,
//! };
//! use molgfx::molframe;
//! // The semantic layer's composition methods resolve through its trait.
//! use molgfx::semantic::FocusScene;
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let structure = molframe::read("1abc.cif")
//!         .map_err(|errors| format!("{} diagnostics", errors.len()))?;
//!     let mut scene = Scene::from_structure(&structure)?;
//!
//!     scene.represent(Select::polymer(), RepresentationKind::Cartoon)?;
//!     scene.represent(Select::ligands(), RepresentationKind::BallAndStick)?;
//!     let ligand = scene.select(Select::ligands())?;
//!     scene.focus(ligand)?;
//!
//!     let camera = Camera::framing_aabb(&scene.world_aabb(), 16.0 / 9.0);
//!     let mut engine = Engine::new(&EngineConfig::default(), None)?;
//!     let image = engine.render_image(
//!         &scene,
//!         &camera,
//!         ImageConfig {
//!             width: 1920,
//!             height: 1080,
//!         },
//!     )?;
//!     std::fs::write("structure.png", image.png_bytes()?)?;
//!     Ok(())
//! }
//! ```

#![forbid(unsafe_code)]

pub mod core;
pub mod gpu;
pub mod math;
#[cfg(feature = "realtime")]
pub mod render;
#[cfg(feature = "semantic")]
pub mod semantic;

pub use molframe;

pub use crate::core::{
    AtomSelection, AttributeHandle, CameraBookmark, ColorScheme, CoreError, DifferenceScene,
    Ensemble, EnsembleHandle, EntityProvenance, InstanceBatchHandle, Material, MeasurementHandle,
    MeshHandle, OverlayHandle, PointBatchHandle, PrimitiveHandle, RelationBatchHandle,
    Representation, RepresentationConfig, RepresentationHandle, RepresentationKind,
    RepresentationPreset, Scene, Select, SelectionHandle, StructureHandle, Timeline,
    TimelineTrackHandle, VolumeHandle,
};
pub use crate::math::{Aabb, Camera, Mat3, Mat4, Quat, Rgba8, Vec2, Vec3, Vec4};
#[cfg(feature = "realtime")]
pub use crate::render::{
    Engine, EngineConfig, FrameReport, Image, ImageConfig, Pick, RenderError, RenderMode,
    RenderProfile,
};
#[cfg(feature = "semantic")]
pub use crate::semantic::{
    FocusScene, FocusStyle, FocusView, GenericCompositionScene, SurfaceZone, SurfaceZoneScene,
    SurfaceZoneStyle,
};
