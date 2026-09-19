//! Typed render-profile adapters split by presentation responsibility.

pub(crate) mod composition;
pub(crate) mod context;
pub(crate) mod display;
pub(crate) mod effects;
pub(crate) mod focus;

pub(crate) use composition::{PyRenderProfile, PyResolvedRenderPlan};
pub(crate) use context::{PyBackdropStyle, PyIllustrationStyle, PyLightingEnvironment};
pub(crate) use display::PyDisplayTransform;
pub(crate) use effects::{
    PyBloomStyle, PyDepthOfField, PyEffectLayer, PyMotionBlur, PyPresentationEffect,
};
pub(crate) use focus::PyFocusTarget;
