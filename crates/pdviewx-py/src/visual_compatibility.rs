//! Drawable-channel compatibility exposed to Python.

use super::PyVisualOutput;
use crate::core::PyRepresentationKind;
use pyo3::prelude::*;

#[pyclass(name = "VisualCompatibility", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyVisualCompatibility(pdviewx::VisualCompatibility);

#[pymethods]
impl PyVisualCompatibility {
    #[staticmethod]
    fn none() -> Self {
        Self(pdviewx::VisualCompatibility::NONE)
    }
    #[staticmethod]
    fn appearance() -> Self {
        Self(pdviewx::VisualCompatibility::APPEARANCE)
    }
    #[staticmethod]
    fn analytic_appearance() -> Self {
        Self(pdviewx::VisualCompatibility::ANALYTIC_APPEARANCE)
    }
    #[staticmethod]
    fn sized() -> Self {
        Self(pdviewx::VisualCompatibility::SIZED)
    }
    #[staticmethod]
    fn deformable() -> Self {
        Self(pdviewx::VisualCompatibility::DEFORMABLE)
    }
    #[staticmethod]
    fn ribbon() -> Self {
        Self(pdviewx::VisualCompatibility::RIBBON)
    }
    #[staticmethod]
    fn for_representation(kind: PyRepresentationKind) -> Self {
        Self(pdviewx::VisualCompatibility::for_representation(
            kind.into(),
        ))
    }
    fn supports(&self, output: PyVisualOutput) -> bool {
        self.0.supports(output.into())
    }
}
