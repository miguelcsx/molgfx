//! Registration hub for the rendering binding modules.

mod brick;
mod brick_atlas;
mod chunk_streaming;
mod config;
mod engine;
mod engine_sequence;
mod entity_kind;
mod image_transfer;
mod profile;
mod session;

pub(crate) use config::*;
pub(crate) use profile::*;

use pyo3::prelude::*;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    brick::register(module)?;
    brick_atlas::register(module)?;
    chunk_streaming::register(module)?;
    config::register(module)?;
    profile::register(module)?;
    session::register(module)?;
    engine::register(module)
}
