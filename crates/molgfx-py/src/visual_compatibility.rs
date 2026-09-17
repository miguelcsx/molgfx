//! Drawable-channel compatibility exposed to Python.

use super::PyVisualOutput;
use crate::core::PyRepresentationKind;
use pyo3::prelude::*;

#[pyclass(name = "VisualCompatibility", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyVisualCompatibility(molgfx::VisualCompatibility);

#[pymethods]
impl PyVisualCompatibility {
    #[staticmethod]
    fn none() -> Self {
        Self(molgfx::VisualCompatibility::NONE)
    }
    #[staticmethod]
    fn appearance() -> Self {
        Self(molgfx::VisualCompatibility::APPEARANCE)
    }
    #[staticmethod]
    fn analytic_appearance() -> Self {
        Self(molgfx::VisualCompatibility::ANALYTIC_APPEARANCE)
    }
    #[staticmethod]
    fn sized() -> Self {
        Self(molgfx::VisualCompatibility::SIZED)
    }
    #[staticmethod]
    fn deformable() -> Self {
        Self(molgfx::VisualCompatibility::DEFORMABLE)
    }
    #[staticmethod]
    fn ribbon() -> Self {
        Self(molgfx::VisualCompatibility::RIBBON)
    }
    #[staticmethod]
    fn for_representation(kind: PyRepresentationKind) -> Self {
        Self(molgfx::VisualCompatibility::for_representation(kind.into()))
    }
    fn supports(&self, output: PyVisualOutput) -> bool {
        self.0.supports(output.into())
    }
}
