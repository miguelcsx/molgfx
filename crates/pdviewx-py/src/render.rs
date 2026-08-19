//! Registration hub for the rendering binding modules.

mod config;
mod engine;
mod profile;
mod session;

pub(crate) use config::*;
pub(crate) use profile::*;

use pyo3::prelude::*;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    config::register(module)?;
    profile::register(module)?;
    session::register(module)?;
    engine::register(module)
}
