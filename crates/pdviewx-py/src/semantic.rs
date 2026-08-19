//! Registration hub for semantic policy adapters.

#[path = "semantic/difference.rs"]
mod difference;
#[path = "semantic/ensemble.rs"]
mod ensemble;
#[path = "semantic/focus.rs"]
mod focus;
#[path = "semantic/streaming.rs"]
mod streaming;

pub(crate) use focus::*;

use pyo3::prelude::*;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    focus::register(module)?;
    difference::register(module)?;
    ensemble::register(module)?;
    streaming::register(module)
}
