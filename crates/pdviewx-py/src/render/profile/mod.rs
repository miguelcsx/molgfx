//! Typed render-profile adapters split by presentation responsibility.

mod composition;
mod context;
mod display;
mod effects;
mod focus;
mod registration;

pub(crate) use composition::{PyRenderProfile, PyResolvedRenderPlan};
pub(crate) use context::{PyBackdropStyle, PyIllustrationStyle, PyLightingEnvironment};
pub(crate) use display::PyDisplayTransform;
pub(crate) use effects::{
    PyBloomStyle, PyDepthOfField, PyEffectLayer, PyMotionBlur, PyPresentationEffect,
};
pub(crate) use focus::PyFocusTarget;
pub(crate) use registration::register;
