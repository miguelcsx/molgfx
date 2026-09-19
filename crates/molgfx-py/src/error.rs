//! Conversion of typed Rust diagnostics to stable Python exceptions.

use pyo3::create_exception;
use pyo3::exceptions::{PyException, PyValueError};
use pyo3::prelude::*;

create_exception!(_engine, MolgfxError, PyException);
create_exception!(_engine, CoreError, MolgfxError);
create_exception!(_engine, GpuError, MolgfxError);
create_exception!(_engine, SemanticError, MolgfxError);
create_exception!(_engine, RenderError, MolgfxError);
create_exception!(_engine, VisualError, MolgfxError);
create_exception!(_engine, ChunkPlacementError, MolgfxError);
create_exception!(_engine, ChunkResidencyError, MolgfxError);
create_exception!(_engine, ManifestError, CoreError);

pub(crate) fn core<T>(result: Result<T, molgfx::core::CoreError>) -> PyResult<T> {
    result.map_err(|error| CoreError::new_err(format!("{}: {error}", error.code())))
}

pub(crate) fn render<T>(result: Result<T, molgfx::render::RenderError>) -> PyResult<T> {
    result.map_err(|error| RenderError::new_err(format!("{}: {error}", error.code())))
}

pub(crate) fn brick_atlas<T>(result: Result<T, molgfx::render::BrickAtlasError>) -> PyResult<T> {
    result.map_err(|error| RenderError::new_err(error.to_string()))
}

pub(crate) fn chunk_placement<T>(
    result: Result<T, molgfx::render::ChunkPlacementError>,
) -> PyResult<T> {
    result.map_err(|error| ChunkPlacementError::new_err(error.to_string()))
}

pub(crate) fn chunk_residency<T>(
    result: Result<T, molgfx::render::ChunkResidencyError>,
) -> PyResult<T> {
    result.map_err(|error| ChunkResidencyError::new_err(error.to_string()))
}

pub(crate) fn dataset<T>(result: Result<T, molgfx::core::DatasetError>) -> PyResult<T> {
    result.map_err(|error| CoreError::new_err(error.to_string()))
}

pub(crate) fn manifest_error(error: molgfx::core::ManifestError) -> PyErr {
    ManifestError::new_err(error.to_string())
}

pub(crate) fn manifest<T>(result: Result<T, molgfx::core::ManifestError>) -> PyResult<T> {
    result.map_err(manifest_error)
}

pub(crate) fn residency<T>(result: Result<T, molgfx::semantic::ResidencyError>) -> PyResult<T> {
    result.map_err(|error| SemanticError::new_err(error.to_string()))
}

pub(crate) fn composition<T>(result: Result<T, molgfx::semantic::CompositionError>) -> PyResult<T> {
    result.map_err(|error| SemanticError::new_err(error.to_string()))
}

pub(crate) fn focus<T>(result: Result<T, molgfx::semantic::FocusError>) -> PyResult<T> {
    result.map_err(|error| SemanticError::new_err(error.to_string()))
}

pub(crate) fn value(message: impl Into<String>) -> PyErr {
    PyValueError::new_err(message.into())
}

pub(crate) fn visual<T>(result: Result<T, molgfx::core::VisualError>) -> PyResult<T> {
    result.map_err(|error| VisualError::new_err(error.to_string()))
}
