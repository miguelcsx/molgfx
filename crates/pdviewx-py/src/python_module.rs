//! Native Python module registration.

use pyo3::prelude::*;

#[pymodule]
pub(crate) fn _native(module: &Bound<'_, PyModule>) -> PyResult<()> {
    crate::generated_registration::register(module)
}
