//! Python adapters for pairwise structural differences.

use crate::core::{
    PyAtomPropertyHandle, PyRepresentationHandle, PyRepresentationKind, PyScene, PySelectionHandle,
    PyStructureHandle,
};
use crate::error::core;
use pyo3::prelude::*;

#[pyclass(name = "AtomCorrespondence", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyAtomCorrespondence(pub(crate) pdviewx::AtomCorrespondence);

#[pymethods]
impl PyAtomCorrespondence {
    #[new]
    fn new(left: u32, right: u32) -> Self {
        Self(pdviewx::AtomCorrespondence { left, right })
    }
    #[getter]
    fn left(&self) -> u32 {
        self.0.left
    }
    #[getter]
    fn right(&self) -> u32 {
        self.0.right
    }
}

#[pyclass(name = "DifferenceStyle", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyDifferenceStyle(pub(crate) pdviewx::DifferenceStyle);

#[pymethods]
impl PyDifferenceStyle {
    #[new]
    #[pyo3(signature = (tolerance_angstrom=0.5, context_opacity=0.16, representation=None))]
    fn new(
        tolerance_angstrom: f32,
        context_opacity: f32,
        representation: Option<PyRepresentationKind>,
    ) -> Self {
        let default = pdviewx::DifferenceStyle::default();
        Self(pdviewx::DifferenceStyle {
            tolerance_angstrom,
            context_opacity,
            representation: representation.map_or(default.representation, Into::into),
        })
    }
    #[staticmethod]
    fn default() -> Self {
        Self(pdviewx::DifferenceStyle::default())
    }
    #[getter]
    fn tolerance_angstrom(&self) -> f32 {
        self.0.tolerance_angstrom
    }
    #[getter]
    fn context_opacity(&self) -> f32 {
        self.0.context_opacity
    }
    #[getter]
    fn representation(&self) -> PyRepresentationKind {
        self.0.representation.into()
    }
}

#[pyclass(name = "DifferenceView", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyDifferenceView {
    properties: [PyAtomPropertyHandle; 2],
    selections: [PySelectionHandle; 2],
    representations: [PyRepresentationHandle; 2],
    maximum_displacement: f32,
}

impl From<pdviewx::DifferenceView> for PyDifferenceView {
    fn from(value: pdviewx::DifferenceView) -> Self {
        Self {
            properties: value.properties.map(Into::into),
            selections: value.selections.map(Into::into),
            representations: value.representations.map(Into::into),
            maximum_displacement: value.maximum_displacement,
        }
    }
}

#[pymethods]
impl PyDifferenceView {
    #[getter]
    fn properties(&self) -> (PyAtomPropertyHandle, PyAtomPropertyHandle) {
        (self.properties[0], self.properties[1])
    }
    #[getter]
    fn selections(&self) -> (PySelectionHandle, PySelectionHandle) {
        (self.selections[0], self.selections[1])
    }
    #[getter]
    fn representations(&self) -> (PyRepresentationHandle, PyRepresentationHandle) {
        (self.representations[0], self.representations[1])
    }
    #[getter]
    fn maximum_displacement(&self) -> f32 {
        self.maximum_displacement
    }
}

#[pymethods]
impl PyScene {
    fn render_difference(
        &mut self,
        structures: (PyStructureHandle, PyStructureHandle),
        correspondence: Vec<PyAtomCorrespondence>,
        provenance: &str,
        style: Option<PyDifferenceStyle>,
    ) -> PyResult<PyDifferenceView> {
        let pairs = correspondence
            .iter()
            .map(|value| value.0)
            .collect::<Vec<_>>();
        let style = style.map_or_else(pdviewx::DifferenceStyle::default, |value| value.0);
        core(pdviewx::DifferenceScene::render_difference(
            &mut self.inner,
            [structures.0.0, structures.1.0],
            &pairs,
            provenance,
            style,
        ))
        .map(Into::into)
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyAtomCorrespondence>()?;
    module.add_class::<PyDifferenceStyle>()?;
    module.add_class::<PyDifferenceView>()
}
