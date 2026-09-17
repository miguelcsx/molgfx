//! Python adapters for level-of-detail and streaming policies.

use crate::core::{PyRepresentationHandle, PyScene, PyStructureHandle};
use crate::error::{core, value};
use crate::math::PyCamera;
use numpy::{PyReadonlyArray1, PyUntypedArrayMethods};
use pyo3::exceptions::PyMemoryError;
use pyo3::prelude::*;

#[pyclass(name = "LodLevel", frozen, eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PyLodLevel {
    Atom,
    Residue,
    SecondaryStructure,
    Domain,
}

impl From<PyLodLevel> for molgfx::LodLevel {
    fn from(value: PyLodLevel) -> Self {
        match value {
            PyLodLevel::Atom => Self::Atom,
            PyLodLevel::Residue => Self::Residue,
            PyLodLevel::SecondaryStructure => Self::SecondaryStructure,
            PyLodLevel::Domain => Self::Domain,
        }
    }
}

impl From<molgfx::LodLevel> for PyLodLevel {
    fn from(value: molgfx::LodLevel) -> Self {
        match value {
            molgfx::LodLevel::Atom => Self::Atom,
            molgfx::LodLevel::Residue => Self::Residue,
            molgfx::LodLevel::SecondaryStructure => Self::SecondaryStructure,
            molgfx::LodLevel::Domain => Self::Domain,
        }
    }
}

#[pyclass(name = "LodPolicy", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyLodPolicy(pub(crate) molgfx::LodPolicy);

#[pymethods]
impl PyLodPolicy {
    #[new]
    #[pyo3(signature = (atom_pixels=4.0, residue_pixels=1.0, secondary_pixels=0.25, hysteresis=0.15))]
    fn new(atom_pixels: f32, residue_pixels: f32, secondary_pixels: f32, hysteresis: f32) -> Self {
        Self(molgfx::LodPolicy {
            atom_pixels,
            residue_pixels,
            secondary_pixels,
            hysteresis,
        })
    }
    #[staticmethod]
    fn default() -> Self {
        Self(molgfx::LodPolicy::default())
    }
    fn select(&self, error_pixels: f32, importance: f32, previous: PyLodLevel) -> PyLodLevel {
        self.0
            .select(error_pixels, importance, previous.into())
            .into()
    }
}

#[pyclass(name = "LodClusterKey", frozen, eq, skip_from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PyLodClusterKey(pub(crate) molgfx::LodClusterKey);

#[pymethods]
impl PyLodClusterKey {
    #[getter]
    fn structure(&self) -> PyStructureHandle {
        self.0.structure.into()
    }
    #[getter]
    fn level(&self) -> PyLodLevel {
        self.0.level.into()
    }
    #[getter]
    fn index(&self) -> u32 {
        self.0.index
    }
}

#[pyclass(name = "LodFrame", skip_from_py_object)]
#[derive(Clone, Debug, Default)]
pub(crate) struct PyLodFrame(pub(crate) molgfx::LodFrame);

#[pymethods]
impl PyLodFrame {
    #[new]
    fn new() -> Self {
        Self::default()
    }
    #[getter]
    fn visible(&self) -> Vec<PyLodClusterKey> {
        self.0
            .visible()
            .iter()
            .copied()
            .map(PyLodClusterKey)
            .collect()
    }
    #[getter]
    fn atom_structures(&self) -> Vec<PyStructureHandle> {
        self.0
            .atom_structures()
            .iter()
            .copied()
            .map(Into::into)
            .collect()
    }
    fn level(&self, structure: PyStructureHandle) -> Option<PyLodLevel> {
        self.0.level(structure.0).map(Into::into)
    }
    #[getter]
    fn levels(&self) -> Vec<(PyStructureHandle, PyLodLevel)> {
        self.0
            .levels()
            .map(|(structure, level)| (structure.into(), level.into()))
            .collect()
    }
}

#[pyclass(name = "LodIndex", frozen, skip_from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyLodIndex(pub(crate) molgfx::LodIndex);

#[pymethods]
impl PyLodIndex {
    #[new]
    fn new(scene: PyRef<'_, PyScene>) -> Self {
        Self(molgfx::LodIndex::from_scene(&scene.inner))
    }
    #[getter]
    fn cluster_count(&self) -> usize {
        self.0.clusters().len()
    }
    #[pyo3(signature = (camera, viewport, output, policy=None, previous=None))]
    fn select_into(
        &self,
        camera: PyCamera,
        viewport: (u32, u32),
        mut output: PyRefMut<'_, PyLodFrame>,
        policy: Option<PyLodPolicy>,
        previous: Option<PyRef<'_, PyLodFrame>>,
    ) {
        let previous = previous.as_ref().map(|frame| &frame.0);
        let policy = match policy {
            Some(policy) => policy.0,
            None => molgfx::LodPolicy::default(),
        };
        self.0.select_into(
            &camera.inner,
            [viewport.0, viewport.1],
            policy,
            previous,
            &mut output.0,
        );
    }
}

#[pyclass(name = "LodScene", skip_from_py_object)]
#[derive(Clone, Debug, Default)]
pub(crate) struct PyLodScene(pub(crate) molgfx::LodScene);

#[pymethods]
impl PyLodScene {
    #[new]
    fn new() -> Self {
        Self::default()
    }
    fn apply(
        &mut self,
        mut scene: PyRefMut<'_, PyScene>,
        index: PyRef<'_, PyLodIndex>,
        frame: PyRef<'_, PyLodFrame>,
    ) -> PyResult<()> {
        core(self.0.apply(&mut scene.inner, &index.0, &frame.0))
    }
    fn apply_transition(
        &mut self,
        mut scene: PyRefMut<'_, PyScene>,
        index: PyRef<'_, PyLodIndex>,
        from_frame: PyRef<'_, PyLodFrame>,
        to_frame: PyRef<'_, PyLodFrame>,
        weight: f32,
    ) -> PyResult<()> {
        core(self.0.apply_transition(
            &mut scene.inner,
            &index.0,
            &from_frame.0,
            &to_frame.0,
            weight,
        ))
    }
    fn bind_detail_representation(
        &mut self,
        scene: PyRef<'_, PyScene>,
        structure: PyStructureHandle,
        representation: PyRepresentationHandle,
    ) -> PyResult<()> {
        core(
            self.0
                .bind_detail_representation(&scene.inner, structure.0, representation.0),
        )
    }
    fn unbind_detail_representation(
        &mut self,
        mut scene: PyRefMut<'_, PyScene>,
        representation: PyRepresentationHandle,
    ) {
        self.0
            .unbind_detail_representation(&mut scene.inner, representation.0);
    }
    fn bind_coarse_representation(
        &mut self,
        scene: PyRef<'_, PyScene>,
        structure: PyStructureHandle,
        representation: PyRepresentationHandle,
    ) -> PyResult<()> {
        core(
            self.0
                .bind_coarse_representation(&scene.inner, structure.0, representation.0),
        )
    }
    fn unbind_coarse_representation(
        &mut self,
        mut scene: PyRefMut<'_, PyScene>,
        representation: PyRepresentationHandle,
    ) {
        self.0
            .unbind_coarse_representation(&mut scene.inner, representation.0);
    }
    #[getter]
    fn primitive_count(&self) -> usize {
        self.0.primitive_count()
    }
    #[getter]
    fn detail_representation_count(&self) -> usize {
        self.0.detail_representation_count()
    }
    #[getter]
    fn coarse_representation_count(&self) -> usize {
        self.0.coarse_representation_count()
    }
}

#[pyclass(name = "StreamingBudget", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyStreamingBudget(pub(crate) molgfx::StreamingBudget);

#[pymethods]
impl PyStreamingBudget {
    #[new]
    #[pyo3(signature = (max_resident_bytes=268_435_456, max_requests_per_frame=64))]
    fn new(max_resident_bytes: u64, max_requests_per_frame: usize) -> Self {
        Self(molgfx::StreamingBudget {
            max_resident_bytes,
            max_requests_per_frame,
        })
    }
    #[staticmethod]
    fn default() -> Self {
        Self(molgfx::StreamingBudget::default())
    }
    #[getter]
    fn max_resident_bytes(&self) -> u64 {
        self.0.max_resident_bytes
    }
    #[getter]
    fn max_requests_per_frame(&self) -> usize {
        self.0.max_requests_per_frame
    }
}

#[pyclass(name = "ChunkKey", frozen, skip_from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyChunkKey(pub(crate) molgfx::ChunkKey);

#[pymethods]
impl PyChunkKey {
    #[getter]
    fn structure(&self) -> PyStructureHandle {
        self.0.structure.into()
    }
    #[getter]
    fn level(&self) -> PyLodLevel {
        self.0.level.into()
    }
    #[getter]
    fn index(&self) -> u32 {
        self.0.index
    }
}

#[pyclass(name = "ChunkRequest", frozen, skip_from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyChunkRequest(pub(crate) molgfx::ChunkRequest);

#[pymethods]
impl PyChunkRequest {
    #[getter]
    fn key(&self) -> PyChunkKey {
        PyChunkKey(self.0.key)
    }
    #[getter]
    fn priority(&self) -> f32 {
        self.0.priority
    }
    #[getter]
    fn bytes(&self) -> u64 {
        self.0.bytes
    }
}

#[pyclass(name = "StreamPlan", skip_from_py_object)]
#[derive(Clone, Debug, Default)]
pub(crate) struct PyStreamPlan(pub(crate) molgfx::StreamPlan);

#[pymethods]
impl PyStreamPlan {
    #[new]
    fn new() -> Self {
        Self::default()
    }
    #[getter]
    fn retain(&self) -> Vec<PyChunkRequest> {
        self.0.retain.iter().copied().map(PyChunkRequest).collect()
    }
    #[getter]
    fn evict(&self) -> Vec<PyChunkKey> {
        self.0.evict.iter().copied().map(PyChunkKey).collect()
    }
    #[getter]
    fn resident_bytes(&self) -> u64 {
        self.0.resident_bytes
    }
}

#[pyclass(name = "StreamPlanner")]
#[derive(Debug)]
pub(crate) struct PyStreamPlanner {
    inner: molgfx::StreamPlanner,
    requests: Vec<molgfx::ChunkRequest>,
}

#[pymethods]
impl PyStreamPlanner {
    #[new]
    #[pyo3(signature = (budget=None))]
    fn new(budget: Option<PyStreamingBudget>) -> Self {
        let budget = match budget {
            Some(value) => value.0,
            None => molgfx::StreamingBudget::default(),
        };
        Self {
            inner: molgfx::StreamPlanner::new(budget),
            requests: Vec::new(),
        }
    }
    #[getter]
    fn budget(&self) -> PyStreamingBudget {
        PyStreamingBudget(self.inner.budget())
    }
    fn copy_plan_from_numpy(
        &mut self,
        structure: PyStructureHandle,
        levels: PyReadonlyArray1<'_, u8>,
        indices: PyReadonlyArray1<'_, u32>,
        priorities: PyReadonlyArray1<'_, f32>,
        bytes: PyReadonlyArray1<'_, u64>,
        mut output: PyRefMut<'_, PyStreamPlan>,
    ) -> PyResult<()> {
        self.copy_request_arrays(structure, levels, indices, priorities, bytes)?;
        self.inner.plan_into(&self.requests, &mut output.0);
        Ok(())
    }
}

impl PyStreamPlanner {
    fn copy_request_arrays(
        &mut self,
        structure: PyStructureHandle,
        levels: PyReadonlyArray1<'_, u8>,
        indices: PyReadonlyArray1<'_, u32>,
        priorities: PyReadonlyArray1<'_, f32>,
        bytes: PyReadonlyArray1<'_, u64>,
    ) -> PyResult<()> {
        let length = levels.shape()[0];
        if indices.shape() != [length]
            || priorities.shape() != [length]
            || bytes.shape() != [length]
        {
            return Err(value("streaming columns must have the same length"));
        }
        let levels = levels
            .as_slice()
            .map_err(|_| value("levels must be C-contiguous"))?;
        let indices = indices
            .as_slice()
            .map_err(|_| value("indices must be C-contiguous"))?;
        let priorities = priorities
            .as_slice()
            .map_err(|_| value("priorities must be C-contiguous"))?;
        let bytes = bytes
            .as_slice()
            .map_err(|_| value("bytes must be C-contiguous"))?;
        self.requests.clear();
        self.reserve_requests(length)?;
        for row in 0..length {
            self.requests.push(molgfx::ChunkRequest {
                key: molgfx::ChunkKey {
                    structure: structure.0,
                    level: lod_level(levels[row])?,
                    index: indices[row],
                },
                priority: priorities[row],
                bytes: bytes[row],
            });
        }
        Ok(())
    }

    fn reserve_requests(&mut self, length: usize) -> PyResult<()> {
        self.requests
            .try_reserve_exact(length)
            .map_err(|_| PyMemoryError::new_err("stream request allocation failed"))
    }
}

fn lod_level(code: u8) -> PyResult<molgfx::LodLevel> {
    match code {
        0 => Ok(molgfx::LodLevel::Atom),
        1 => Ok(molgfx::LodLevel::Residue),
        2 => Ok(molgfx::LodLevel::SecondaryStructure),
        3 => Ok(molgfx::LodLevel::Domain),
        _ => Err(value("LOD level codes must be in [0, 3]")),
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyLodLevel>()?;
    module.add_class::<PyLodPolicy>()?;
    module.add_class::<PyLodClusterKey>()?;
    module.add_class::<PyLodFrame>()?;
    module.add_class::<PyLodIndex>()?;
    module.add_class::<PyLodScene>()?;
    module.add_class::<PyStreamingBudget>()?;
    module.add_class::<PyChunkKey>()?;
    module.add_class::<PyChunkRequest>()?;
    module.add_class::<PyStreamPlan>()?;
    module.add_class::<PyStreamPlanner>()
}
