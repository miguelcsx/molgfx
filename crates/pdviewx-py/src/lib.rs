//! Thin Python bindings over the public pdviewx facade.
//!
//! This crate contains only `PyO3` type adapters, argument marshalling and
//! registration. Scene queries, validation, rendering and serialization stay
//! in the Rust crates so every host language follows one implementation.

#![forbid(unsafe_code)]

mod annotations;
mod core;
mod error;
mod interactions;
mod math;
mod render;
mod semantic;
mod trajectory;
mod values;

use pyo3::prelude::*;

#[pymodule]
fn _native(module: &Bound<'_, PyModule>) -> PyResult<()> {
    error::register(module)?;
    math::register(module)?;
    core::register(module)?;
    annotations::register(module)?;
    interactions::register(module)?;
    semantic::register(module)?;
    trajectory::register(module)?;
    values::register(module)?;
    render::register(module)
}
