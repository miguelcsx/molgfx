//! Browser bindings over the same declarative scene and WebGPU engine.

#![forbid(unsafe_code)]

#[cfg(target_arch = "wasm32")]
mod browser;
#[cfg(target_arch = "wasm32")]
mod browser_camera;
#[cfg(target_arch = "wasm32")]
mod browser_controls;
#[cfg(target_arch = "wasm32")]
mod browser_engine;
#[cfg(target_arch = "wasm32")]
mod browser_frame;
#[cfg(target_arch = "wasm32")]
mod browser_presentation;
#[cfg(target_arch = "wasm32")]
mod browser_secondary;
#[cfg(target_arch = "wasm32")]
mod browser_types;
#[cfg(target_arch = "wasm32")]
mod generic_batches;
#[cfg(target_arch = "wasm32")]
mod generic_compositions;
#[cfg(target_arch = "wasm32")]
mod out_of_core;
#[cfg(target_arch = "wasm32")]
mod timeline;

#[cfg(target_arch = "wasm32")]
pub use browser::{
    WebCamera, WebCameraController, WebCameraControllerKind, WebCapabilities, WebClipCap,
    WebClipPlane, WebClipSet, WebEngine, WebEngineConfig, WebFrameReport, WebFrameTiming,
    WebLifecycleState, WebMaterial, WebMaterialModel, WebNavigationKey, WebPick, WebPickKind,
    WebPointerButton, WebPowerPreference, WebProfile, WebRenderMode, WebRepresentation,
    WebRepresentationKind, WebResolutionPolicy, WebScene,
};
#[cfg(target_arch = "wasm32")]
pub use generic_batches::{
    WebAnalyticTemplate, WebAttributeHandle, WebInstanceBatchHandle, WebPointBatchHandle,
    WebRelationBatchHandle, WebRowDomain,
};
#[cfg(target_arch = "wasm32")]
pub use generic_compositions::{
    WebDifferenceComposition, WebEnsembleComposition, WebGenericCompositionView,
};
#[cfg(target_arch = "wasm32")]
pub use out_of_core::{WebChunkFootprint, WebChunkId, WebDatasetId, WebLocalRow, WebLogicalRow};
#[cfg(target_arch = "wasm32")]
pub use timeline::{WebPlaybackMode, WebTimeWarp, WebTimeline, WebTimelineTrackHandle};
