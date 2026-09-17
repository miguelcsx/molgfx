//! Python adapter for the mutable declarative scene model.

use super::atom_property::PyAtomProperty;
use super::{
    PyAtomPropertyHandle, PyOccupancyStream, PyRepresentation, PyRepresentationHandle,
    PyRepresentationKind, PyRepresentationPreset, PyScalarVolume, PySegmentationHandle,
    PySegmentedVolume, PySelect, PySelectionHandle, PyStructureHandle, PyVolumeHandle,
};
use crate::error::{core, manifest, value};
use crate::topology::PyBondTopologySegment;
use crate::trajectory::PyTrajectorySegment;
use crate::visual::PyVisualStyle;
use numpy::PyReadonlyArray1;
use pyo3::prelude::*;
use pyo3::types::PyBytes;

#[pyclass(name = "Scene")]
#[derive(Debug)]
pub(crate) struct PyScene {
    pub(crate) inner: molgfx::Scene,
}

impl PyScene {
    fn target_from_python(
        &mut self,
        object: &Bound<'_, PyAny>,
    ) -> PyResult<molgfx::RepresentationInput> {
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
    ) -> PyResult<molgfx::SelectionHandle> {
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

    fn occupancy_mask_from_python(
        &mut self,
        structure: molgfx::StructureHandle,
        object: &Bound<'_, PyAny>,
    ) -> PyResult<molgfx::AtomSelection> {
        let selection = self.selection_from_python(object)?;
        self.inner
            .selection_for(selection, structure)
            .cloned()
            .ok_or_else(|| value("selection does not apply to the structure"))
    }
}

#[pymethods]
impl PyScene {
    #[new]
    fn new() -> Self {
        Self {
            inner: molgfx::Scene::new(),
        }
    }

    #[staticmethod]
    fn from_structure_shared(object: &Bound<'_, PyAny>) -> PyResult<Self> {
        let structure = pdbiox_py::structure_from_python(object)?;
        core(molgfx::Scene::from_structure(&structure)).map(|inner| Self { inner })
    }

    #[staticmethod]
    #[pyo3(signature = (source, structures, volumes=None, segmentations=None))]
    fn from_manifest_json(
        source: &Bound<'_, PyBytes>,
        structures: &Bound<'_, PyAny>,
        volumes: Option<Vec<PyScalarVolume>>,
        segmentations: Option<Vec<PySegmentedVolume>>,
    ) -> PyResult<Self> {
        let mut rust_structures = Vec::new();
        for item in structures.try_iter()? {
            rust_structures.push(pdbiox_py::structure_from_python(&item?)?);
        }
        let volumes = volumes
            .into_iter()
            .flatten()
            .map(|value| value.0)
            .collect::<Vec<_>>();
        let segmentations = segmentations
            .into_iter()
            .flatten()
            .map(|value| value.0)
            .collect::<Vec<_>>();
        let manifest = manifest(molgfx::read_manifest(source.as_bytes()))?;
        core(molgfx::Scene::from_description(
            &manifest.scene,
            molgfx::SceneDescriptionSources {
                structures: &rust_structures,
                volumes: &volumes,
                segmentations: &segmentations,
                atom_properties: &[],
                meshes: &[],
            },
        ))
        .map(|inner| Self { inner })
    }

    fn add_structure_shared(&mut self, object: &Bound<'_, PyAny>) -> PyResult<PyStructureHandle> {
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
            molgfx::RepresentationTarget::Selection(selection),
            preset.0,
        ))
        .map(|values| values.into_iter().map(Into::into).collect())
    }

    fn add_volume(&mut self, volume: PyScalarVolume) -> PyVolumeHandle {
        self.inner.add_volume(volume.0).into()
    }
    fn add_occupancy_stream(
        &mut self,
        structure: PyStructureHandle,
        selection: &Bound<'_, PyAny>,
        stream: PyOccupancyStream,
    ) -> PyResult<PyVolumeHandle> {
        let mask = self.occupancy_mask_from_python(structure.0, selection)?;
        core(
            self.inner
                .add_occupancy_stream(structure.0, &mask, stream.0),
        )
        .map(Into::into)
    }
    fn replace_occupancy_stream(
        &mut self,
        handle: PyVolumeHandle,
        structure: PyStructureHandle,
        selection: &Bound<'_, PyAny>,
        stream: PyOccupancyStream,
    ) -> PyResult<()> {
        let mask = self.occupancy_mask_from_python(structure.0, selection)?;
        core(
            self.inner
                .replace_occupancy_stream(handle.0, structure.0, &mask, stream.0),
        )
    }
    fn add_segmented_volume(&mut self, volume: PySegmentedVolume) -> PySegmentationHandle {
        self.inner.add_segmented_volume(volume.0).into()
    }
    fn add_atom_property(&mut self, property: PyAtomProperty) -> PyResult<PyAtomPropertyHandle> {
        core(self.inner.add_atom_property(property.0)).map(Into::into)
    }
    fn interpolate_atom_property_shared(
        &mut self,
        property: PyAtomPropertyHandle,
        start: PyReadonlyArray1<'_, f32>,
        end: PyReadonlyArray1<'_, f32>,
        alpha: f32,
    ) -> PyResult<()> {
        let start = start
            .as_slice()
            .map_err(|_| value("start must be C-contiguous float32"))?;
        let end = end
            .as_slice()
            .map_err(|_| value("end must be C-contiguous float32"))?;
        core(
            self.inner
                .interpolate_atom_property(property.0, start, end, alpha),
        )
    }
    fn representation_kind(
        &self,
        representation: PyRepresentationHandle,
    ) -> Option<PyRepresentationKind> {
        self.inner
            .representation(representation.0)
            .map(|value| PyRepresentation::new(value.kind).kind_value())
    }
    fn set_representation_visual(
        &mut self,
        representation: PyRepresentationHandle,
        style: Option<PyVisualStyle>,
    ) -> PyResult<()> {
        core(
            self.inner
                .set_representation_visual(representation.0, style.map(|value| value.0)),
        )
    }
    fn set_visual_scalar_parameter(
        &mut self,
        representation: PyRepresentationHandle,
        index: usize,
        value_: f32,
    ) -> PyResult<()> {
        core(
            self.inner
                .set_visual_scalar_parameter(representation.0, index, value_),
        )
    }
    fn set_visual_color_parameter(
        &mut self,
        representation: PyRepresentationHandle,
        index: usize,
        value_: (f32, f32, f32, f32),
    ) -> PyResult<()> {
        core(self.inner.set_visual_color_parameter(
            representation.0,
            index,
            [value_.0, value_.1, value_.2, value_.3],
        ))
    }
    fn set_visual_vector_parameter(
        &mut self,
        representation: PyRepresentationHandle,
        index: usize,
        value_: (f32, f32, f32),
    ) -> PyResult<()> {
        core(self.inner.set_visual_vector_parameter(
            representation.0,
            index,
            [value_.0, value_.1, value_.2],
        ))
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
    fn set_presentation_time(&mut self, seconds: f64) -> PyResult<()> {
        core(self.inner.set_presentation_time(seconds))
    }
    #[getter]
    fn presentation_time_seconds(&self) -> f32 {
        self.inner.presentation_time_seconds()
    }
    #[getter]
    fn presentation_revision(&self) -> u64 {
        self.inner.presentation_revision()
    }
    fn set_bond_topology_segment(
        &mut self,
        structure: PyStructureHandle,
        segment: PyBondTopologySegment,
    ) -> PyResult<()> {
        core(self.inner.set_bond_topology_segment(structure.0, segment.0))
    }
    fn set_bond_topology_time(
        &mut self,
        structure: PyStructureHandle,
        seconds: f32,
    ) -> PyResult<()> {
        core(self.inner.set_bond_topology_time(structure.0, seconds))
    }
    fn clear_bond_topology(&mut self, structure: PyStructureHandle) -> PyResult<bool> {
        core(self.inner.clear_bond_topology(structure.0))
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
    fn copy_manifest_json<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyBytes>> {
        let manifest_value = self.inner.manifest(Vec::new());
        let mut encoded = Vec::new();
        manifest(molgfx::write_manifest(&mut encoded, &manifest_value))?;
        Ok(PyBytes::new(py, &encoded))
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
