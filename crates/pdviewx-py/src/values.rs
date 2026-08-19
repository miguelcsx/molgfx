//! Registration hub for reusable core value adapters.

#[path = "values/color.rs"]
mod color;
#[path = "values/volume.rs"]
mod volume;

pub(crate) use color::*;
pub(crate) use volume::*;

use pyo3::prelude::*;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    color::register(module)?;
    volume::register(module)
}
