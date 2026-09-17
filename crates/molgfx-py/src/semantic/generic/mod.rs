//! Thin Python adapters for meaning-agnostic domain compositions.

mod model;
mod registration;
mod scene;

pub(crate) use model::PyGenericCompositionView;
pub(crate) use registration::register;
