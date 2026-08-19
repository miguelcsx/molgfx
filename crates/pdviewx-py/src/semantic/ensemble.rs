//! Python adapters for ensembles and probability-cloud views.

use crate::core::PyStructureHandle;
use crate::core::{
    PyEnsembleHandle, PyRepresentationHandle, PyRepresentationKind, PyScene, PySelectionHandle,
    PyVolumeHandle,
};
use crate::error::core;
use pyo3::prelude::*;
use std::sync::Arc;

#[pyclass(name = "Ensemble", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyEnsemble(pub(crate) pdviewx::Ensemble);

#[pymethods]
impl PyEnsemble {
    #[new]
    fn new(members: Vec<PyStructureHandle>, weights: Vec<f32>, provenance: &str) -> PyResult<Self> {
        let members = members.into_iter().map(|value| value.0).collect::<Vec<_>>();
        core(pdviewx::Ensemble::new(
            Arc::from(members.into_boxed_slice()),
            &weights,
            provenance,
        ))
        .map(Self)
    }
    #[getter]
    fn members(&self) -> Vec<PyStructureHandle> {
        self.0.members().iter().copied().map(Into::into).collect()
    }
    #[getter]
    fn weights(&self) -> Vec<f32> {
        self.0.weights().to_vec()
    }
    #[getter]
    fn provenance(&self) -> String {
        self.0.provenance().to_owned()
    }
    #[getter]
    fn dominant(&self) -> usize {
        self.0.dominant_index()
    }
}

#[pyclass(name = "EnsembleStyle", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyEnsembleStyle(pub(crate) pdviewx::EnsembleStyle);

#[pymethods]
impl PyEnsembleStyle {
    #[new]
    #[pyo3(signature = (representation=None, alternate_opacity=0.55, minimum_opacity=0.08))]
    fn new(
        representation: Option<PyRepresentationKind>,
        alternate_opacity: f32,
        minimum_opacity: f32,
    ) -> Self {
        let default = pdviewx::EnsembleStyle::default();
        Self(pdviewx::EnsembleStyle {
            representation: representation.map_or(default.representation, Into::into),
            alternate_opacity,
            minimum_opacity,
        })
    }
    #[staticmethod]
    fn default() -> Self {
        Self(pdviewx::EnsembleStyle::default())
    }
}

#[pyclass(name = "EnsembleView", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyEnsembleView {
    ensemble: PyEnsembleHandle,
    selections: Vec<PySelectionHandle>,
    representations: Vec<PyRepresentationHandle>,
    dominant: usize,
}

impl From<pdviewx::EnsembleView> for PyEnsembleView {
    fn from(value: pdviewx::EnsembleView) -> Self {
        Self {
            ensemble: value.ensemble.into(),
            selections: value.selections.into_iter().map(Into::into).collect(),
            representations: value.representations.into_iter().map(Into::into).collect(),
            dominant: value.dominant,
        }
    }
}

#[pymethods]
impl PyEnsembleView {
    #[getter]
    fn ensemble(&self) -> PyEnsembleHandle {
        self.ensemble
    }
    #[getter]
    fn selections(&self) -> Vec<PySelectionHandle> {
        self.selections.clone()
    }
    #[getter]
    fn representations(&self) -> Vec<PyRepresentationHandle> {
        self.representations.clone()
    }
    #[getter]
    fn dominant(&self) -> usize {
        self.dominant
    }
}

#[pyclass(name = "ProbabilityCloudView", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyProbabilityCloudView {
    ensemble: PyEnsembleHandle,
    volume: PyVolumeHandle,
    representation: PyRepresentationHandle,
}

impl From<pdviewx::ProbabilityCloudView> for PyProbabilityCloudView {
    fn from(value: pdviewx::ProbabilityCloudView) -> Self {
        Self {
            ensemble: value.ensemble.into(),
            volume: value.volume.into(),
            representation: value.representation.into(),
        }
    }
}

#[pymethods]
impl PyProbabilityCloudView {
    #[getter]
    fn ensemble(&self) -> PyEnsembleHandle {
        self.ensemble
    }
    #[getter]
    fn volume(&self) -> PyVolumeHandle {
        self.volume
    }
    #[getter]
    fn representation(&self) -> PyRepresentationHandle {
        self.representation
    }
}

#[pymethods]
impl PyScene {
    fn add_ensemble(&mut self, ensemble: PyEnsemble) -> PyResult<PyEnsembleHandle> {
        core(self.inner.add_ensemble(ensemble.0)).map(Into::into)
    }

    fn overlay_ensemble(
        &mut self,
        ensemble: PyEnsembleHandle,
        style: Option<PyEnsembleStyle>,
    ) -> PyResult<PyEnsembleView> {
        let style = style.map_or_else(pdviewx::EnsembleStyle::default, |value| value.0);
        core(pdviewx::EnsembleScene::overlay_ensemble(
            &mut self.inner,
            ensemble.0,
            style,
        ))
        .map(Into::into)
    }

    fn probability_cloud(
        &mut self,
        ensemble: PyEnsembleHandle,
        volume: PyVolumeHandle,
    ) -> PyResult<PyProbabilityCloudView> {
        core(pdviewx::EnsembleScene::probability_cloud(
            &mut self.inner,
            ensemble.0,
            volume.0,
        ))
        .map(Into::into)
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyEnsemble>()?;
    module.add_class::<PyEnsembleStyle>()?;
    module.add_class::<PyEnsembleView>()?;
    module.add_class::<PyProbabilityCloudView>()
}
