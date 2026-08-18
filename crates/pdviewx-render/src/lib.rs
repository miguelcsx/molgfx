//! The render graph, passes and the engine that drives a frame.
//!
//! Public surface: the engine, its configuration, render targets and typed
//! outcomes. Graph internals, passes, pipelines and culling are not exposed;
//! callers drive rendering entirely through [`Engine`].

#![forbid(unsafe_code)]

mod engine;
mod error;
mod graph;
mod passes;
mod scene_gpu;

#[cfg(test)]
mod testing;

pub use engine::{
    BackdropStyle, BloomStyle, DepthOfField, DisplayGamut, DisplayTransform, EffectLayer, Engine,
    EngineConfig, FocusTarget, FrameOutcome, FrameTiming, IllustrationStyle, Image, ImageConfig,
    LightingEnvironment, MotionBlur, Pick, PickEntity, PresentationEffect, RenderMode,
    RenderProfile, RenderSession, ResolvedRenderPlan, ToneMapping, TransferFunction,
};
pub use error::RenderError;
