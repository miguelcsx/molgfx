//! Drawable-channel compatibility exposed to Python.

use super::enums::PyVisualOutput;
use crate::core::PyRepresentationKind;
use pyo3::prelude::*;

#[pyclass(name = "VisualCompatibility", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyVisualCompatibility(molgfx::core::VisualCompatibility);

#[pymethods]
impl PyVisualCompatibility {
    #[staticmethod]
    fn none() -> Self {
        Self(molgfx::core::VisualCompatibility::NONE)
    }
    #[staticmethod]
    fn appearance() -> Self {
        Self(molgfx::core::VisualCompatibility::APPEARANCE)
    }
    #[staticmethod]
    fn analytic_appearance() -> Self {
        Self(molgfx::core::VisualCompatibility::ANALYTIC_APPEARANCE)
    }
    #[staticmethod]
    fn sized() -> Self {
        Self(molgfx::core::VisualCompatibility::SIZED)
    }
    #[staticmethod]
    fn deformable() -> Self {
        Self(molgfx::core::VisualCompatibility::DEFORMABLE)
    }
    #[staticmethod]
    fn ribbon() -> Self {
        Self(molgfx::core::VisualCompatibility::RIBBON)
    }
    #[staticmethod]
    fn for_representation(kind: PyRepresentationKind) -> Self {
        Self(molgfx::core::VisualCompatibility::for_representation(
            kind.into(),
        ))
    }
    fn supports(&self, output: PyVisualOutput) -> bool {
        self.0.supports(output.into())
    }
}
