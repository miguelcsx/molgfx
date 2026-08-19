//! Python adapter for the mutable declarative scene model.

use super::{
    PyDensityVolume, PyRepresentation, PyRepresentationHandle, PyRepresentationKind,
    PyRepresentationPreset, PySegmentationHandle, PySegmentedVolume, PySelect, PySelectionHandle,
    PyStructureHandle, PyVolumeHandle,
};
use crate::error::{core, value};
use crate::semantic::PyFocusView;
use crate::trajectory::PyTrajectorySegment;
use pyo3::prelude::*;

#[pyclass(name = "Scene")]
#[derive(Debug)]
pub(crate) struct PyScene {
    pub(crate) inner: pdviewx::Scene,
}

impl PyScene {
    fn target_from_python(
        &mut self,
        object: &Bound<'_, PyAny>,
    ) -> PyResult<pdviewx::RepresentationInput> {
        if let Ok(handle) = object.extract::<PyRef<'_, PyVolumeHandle>>() {
            return Ok(handle.0.into());
        }
        if let Ok(handle) = object.extract::<PyRef<'_, PySegmentationHandle>>() {
            return Ok(handle.0.into());
        }
        self.selection_from_python(object).map(Into::into)
    }

    fn selection_from_python(
        &mut self,
        object: &Bound<'_, PyAny>,
    ) -> PyResult<pdviewx::SelectionHandle> {
        if let Ok(handle) = object.extract::<PyRef<'_, PySelectionHandle>>() {
            return Ok(handle.0);
        }
        if let Ok(query) = object.extract::<PyRef<'_, PySelect>>() {
            return core(self.inner.select(query.0.clone()));
        }
        if let Ok(source) = object.extract::<String>() {
            return core(self.inner.select_str(&source));
        }
        Err(value("expected a Select, SelectionHandle, or query string"))
    }
}

#[pymethods]
impl PyScene {
    #[new]
    fn new() -> Self {
        Self {
            inner: pdviewx::Scene::new(),
        }
    }

    #[staticmethod]
    fn from_structure(object: &Bound<'_, PyAny>) -> PyResult<Self> {
        let structure = pdbiox_py::structure_from_python(object)?;
        core(pdviewx::Scene::from_structure(&structure)).map(|inner| Self { inner })
    }

    fn add_structure(&mut self, object: &Bound<'_, PyAny>) -> PyResult<PyStructureHandle> {
        let structure = pdbiox_py::structure_from_python(object)?;
        core(self.inner.add_structure(&structure)).map(Into::into)
    }

    fn select(&mut self, query: PySelect) -> PyResult<PySelectionHandle> {
        core(self.inner.select(query.0)).map(Into::into)
    }
    fn select_str(&mut self, source: &str) -> PyResult<PySelectionHandle> {
        core(self.inner.select_str(source)).map(Into::into)
    }

    fn represent(
        &mut self,
        target: &Bound<'_, PyAny>,
        representation: PyRepresentation,
    ) -> PyResult<PyRepresentationHandle> {
        let target = self.target_from_python(target)?;
        core(self.inner.represent(target, representation.config)).map(Into::into)
    }

    fn represent_preset(
        &mut self,
        selection: &Bound<'_, PyAny>,
        preset: PyRepresentationPreset,
    ) -> PyResult<Vec<PyRepresentationHandle>> {
        let selection = self.selection_from_python(selection)?;
        core(self.inner.represent_preset(
            pdviewx::RepresentationTarget::Selection(selection),
            preset.0,
        ))
        .map(|values| values.into_iter().map(Into::into).collect())
    }

    fn add_volume(&mut self, volume: PyDensityVolume) -> PyVolumeHandle {
        self.inner.add_volume(volume.0).into()
    }
    fn add_segmented_volume(&mut self, volume: PySegmentedVolume) -> PySegmentationHandle {
        self.inner.add_segmented_volume(volume.0).into()
    }
    fn representation_kind(
        &self,
        representation: PyRepresentationHandle,
    ) -> Option<PyRepresentationKind> {
        self.inner
            .representation(representation.0)
            .map(|value| PyRepresentation::new(value.kind).kind_value())
    }

    fn set_trajectory_segment(
        &mut self,
        structure: PyStructureHandle,
        segment: PyTrajectorySegment,
    ) -> PyResult<()> {
        core(self.inner.set_trajectory_segment(structure.0, segment.0))
    }
    fn set_trajectory_time(&mut self, structure: PyStructureHandle, seconds: f32) -> PyResult<()> {
        core(self.inner.set_trajectory_time(structure.0, seconds))
    }
    fn clear_trajectory(&mut self, structure: PyStructureHandle) -> PyResult<bool> {
        core(self.inner.clear_trajectory(structure.0))
    }
    fn focus(&mut self, selection: &Bound<'_, PyAny>) -> PyResult<PyFocusView> {
        let selection = self.selection_from_python(selection)?;
        core(pdviewx::FocusScene::focus(&mut self.inner, selection)).map(Into::into)
    }

    fn hide(&mut self, representation: PyRepresentationHandle) {
        self.inner.hide(representation.0);
    }
    fn show(&mut self, representation: PyRepresentationHandle) {
        self.inner.show(representation.0);
    }
    fn remove_representation(&mut self, representation: PyRepresentationHandle) {
        self.inner.remove_representation(representation.0);
    }

    #[getter]
    fn representation_count(&self) -> usize {
        self.inner.representation_count()
    }
    #[getter]
    fn structure_count(&self) -> usize {
        self.inner.structures().count()
    }
    #[getter]
    fn representation_revision(&self) -> u64 {
        self.inner.representation_revision()
    }
    fn to_json(&self) -> PyResult<String> {
        core(self.inner.to_json())
    }
    fn __repr__(&self) -> String {
        format!(
            "Scene(structures={}, representations={})",
            self.inner.structures().count(),
            self.inner.representation_count()
        )
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyScene>()
}
