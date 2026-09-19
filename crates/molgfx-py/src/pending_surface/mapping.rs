//! Reversible property mapping delegated to Rust.

use pyo3::create_exception;
use pyo3::prelude::*;

create_exception!(_engine, MappingError, crate::error::SemanticError);

#[pyclass(name = "PropertyMapping", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyPropertyMapping(pub(crate) molgfx::semantic::PropertyMapping);

#[pymethods]
impl PyPropertyMapping {
    #[new]
    fn new(domain: [f32; 2], visual: [f32; 2]) -> PyResult<Self> {
        molgfx::semantic::PropertyMapping::new(domain, visual)
            .map(Self)
            .map_err(|error| MappingError::new_err(error.to_string()))
    }

    fn map(&self, value: f32) -> f32 {
        self.0.map(value)
    }

    fn unmap(&self, value: f32) -> f32 {
        self.0.unmap(value)
    }
}
