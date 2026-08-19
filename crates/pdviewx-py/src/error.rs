//! Conversion of typed Rust diagnostics to stable Python exceptions.

use pyo3::create_exception;
use pyo3::exceptions::{PyException, PyValueError};
use pyo3::prelude::*;

create_exception!(_native, PdviewxError, PyException);
create_exception!(_native, CoreError, PdviewxError);
create_exception!(_native, GpuError, PdviewxError);
create_exception!(_native, SemanticError, PdviewxError);
create_exception!(_native, RenderError, PdviewxError);

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add("PdviewxError", module.py().get_type::<PdviewxError>())?;
    module.add("CoreError", module.py().get_type::<CoreError>())?;
    module.add("GpuError", module.py().get_type::<GpuError>())?;
    module.add("SemanticError", module.py().get_type::<SemanticError>())?;
    module.add("RenderError", module.py().get_type::<RenderError>())
}

pub(crate) fn core<T>(result: Result<T, pdviewx::CoreError>) -> PyResult<T> {
    result.map_err(|error| CoreError::new_err(format!("{}: {error}", error.code())))
}

pub(crate) fn render<T>(result: Result<T, pdviewx::RenderError>) -> PyResult<T> {
    result.map_err(|error| RenderError::new_err(format!("{}: {error}", error.code())))
}

pub(crate) fn semantic<T>(result: Result<T, pdviewx::FocusError>) -> PyResult<T> {
    result.map_err(|error| SemanticError::new_err(error.to_string()))
}

pub(crate) fn value(message: impl Into<String>) -> PyErr {
    PyValueError::new_err(message.into())
}
