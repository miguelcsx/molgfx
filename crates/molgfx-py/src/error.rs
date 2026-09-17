//! Conversion of typed Rust diagnostics to stable Python exceptions.

use pyo3::create_exception;
use pyo3::exceptions::{PyException, PyValueError};
use pyo3::prelude::*;

create_exception!(_native, MolgfxError, PyException);
create_exception!(_native, CoreError, MolgfxError);
create_exception!(_native, GpuError, MolgfxError);
create_exception!(_native, SemanticError, MolgfxError);
create_exception!(_native, RenderError, MolgfxError);
create_exception!(_native, VisualError, MolgfxError);

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add("MolgfxError", module.py().get_type::<MolgfxError>())?;
    module.add("CoreError", module.py().get_type::<CoreError>())?;
    module.add("GpuError", module.py().get_type::<GpuError>())?;
    module.add("SemanticError", module.py().get_type::<SemanticError>())?;
    module.add("RenderError", module.py().get_type::<RenderError>())?;
    module.add("VisualError", module.py().get_type::<VisualError>())
}

pub(crate) fn core<T>(result: Result<T, molgfx::CoreError>) -> PyResult<T> {
    result.map_err(|error| CoreError::new_err(format!("{}: {error}", error.code())))
}

pub(crate) fn render<T>(result: Result<T, molgfx::RenderError>) -> PyResult<T> {
    result.map_err(|error| RenderError::new_err(format!("{}: {error}", error.code())))
}

pub(crate) fn brick_atlas<T>(result: Result<T, molgfx::BrickAtlasError>) -> PyResult<T> {
    result.map_err(|error| RenderError::new_err(error.to_string()))
}

pub(crate) fn dataset<T>(result: Result<T, molgfx::DatasetError>) -> PyResult<T> {
    result.map_err(|error| CoreError::new_err(error.to_string()))
}

pub(crate) fn manifest<T>(result: Result<T, molgfx::ManifestError>) -> PyResult<T> {
    result.map_err(|error| CoreError::new_err(error.to_string()))
}

pub(crate) fn residency<T>(result: Result<T, molgfx::ResidencyError>) -> PyResult<T> {
    result.map_err(|error| SemanticError::new_err(error.to_string()))
}

pub(crate) fn value(message: impl Into<String>) -> PyErr {
    PyValueError::new_err(message.into())
}

pub(crate) fn visual<T>(result: Result<T, molgfx::VisualError>) -> PyResult<T> {
    result.map_err(|error| VisualError::new_err(error.to_string()))
}
