//! Registration for the render-profile adapters.

use super::{composition, context, display, effects, focus};
use pyo3::prelude::*;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    display::register(module)?;
    focus::register(module)?;
    context::register(module)?;
    effects::register(module)?;
    composition::register(module)
}
