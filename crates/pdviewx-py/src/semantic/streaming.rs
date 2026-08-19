//! Python adapters for level-of-detail and streaming policies.

use crate::core::PyStructureHandle;
use pyo3::prelude::*;

#[pyclass(name = "LodLevel", frozen, eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PyLodLevel {
    Atom,
    Residue,
    SecondaryStructure,
    Domain,
}

impl From<PyLodLevel> for pdviewx::LodLevel {
    fn from(value: PyLodLevel) -> Self {
        match value {
            PyLodLevel::Atom => Self::Atom,
            PyLodLevel::Residue => Self::Residue,
            PyLodLevel::SecondaryStructure => Self::SecondaryStructure,
            PyLodLevel::Domain => Self::Domain,
        }
    }
}

impl From<pdviewx::LodLevel> for PyLodLevel {
    fn from(value: pdviewx::LodLevel) -> Self {
        match value {
            pdviewx::LodLevel::Atom => Self::Atom,
            pdviewx::LodLevel::Residue => Self::Residue,
            pdviewx::LodLevel::SecondaryStructure => Self::SecondaryStructure,
            pdviewx::LodLevel::Domain => Self::Domain,
        }
    }
}

#[pyclass(name = "LodPolicy", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyLodPolicy(pub(crate) pdviewx::LodPolicy);

#[pymethods]
impl PyLodPolicy {
    #[new]
    #[pyo3(signature = (atom_pixels=4.0, residue_pixels=1.0, secondary_pixels=0.25, hysteresis=0.15))]
    fn new(atom_pixels: f32, residue_pixels: f32, secondary_pixels: f32, hysteresis: f32) -> Self {
        Self(pdviewx::LodPolicy {
            atom_pixels,
            residue_pixels,
            secondary_pixels,
            hysteresis,
        })
    }
    #[staticmethod]
    fn default() -> Self {
        Self(pdviewx::LodPolicy::default())
    }
    fn select(&self, error_pixels: f32, importance: f32, previous: PyLodLevel) -> PyLodLevel {
        self.0
            .select(error_pixels, importance, previous.into())
            .into()
    }
}

#[pyclass(name = "StreamingBudget", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyStreamingBudget(pub(crate) pdviewx::StreamingBudget);

#[pymethods]
impl PyStreamingBudget {
    #[new]
    #[pyo3(signature = (max_resident_bytes=268_435_456, max_requests_per_frame=64))]
    fn new(max_resident_bytes: u64, max_requests_per_frame: usize) -> Self {
        Self(pdviewx::StreamingBudget {
            max_resident_bytes,
            max_requests_per_frame,
        })
    }
    #[staticmethod]
    fn default() -> Self {
        Self(pdviewx::StreamingBudget::default())
    }
}

#[pyclass(name = "ChunkKey", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyChunkKey(pub(crate) pdviewx::ChunkKey);

#[pymethods]
impl PyChunkKey {
    #[new]
    fn new(structure: PyStructureHandle, level: PyLodLevel, index: u32) -> Self {
        Self(pdviewx::ChunkKey {
            structure: structure.0,
            level: level.into(),
            index,
        })
    }
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

#[pyclass(name = "ChunkRequest", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyChunkRequest(pub(crate) pdviewx::ChunkRequest);

#[pymethods]
impl PyChunkRequest {
    #[new]
    fn new(key: PyChunkKey, priority: f32, bytes: u64) -> Self {
        Self(pdviewx::ChunkRequest {
            key: key.0,
            priority,
            bytes,
        })
    }
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

#[pyclass(name = "StreamPlan", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyStreamPlan(pub(crate) pdviewx::StreamPlan);

#[pymethods]
impl PyStreamPlan {
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

#[pyclass(name = "StreamPlanner", from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyStreamPlanner(pub(crate) pdviewx::StreamPlanner);

#[pymethods]
impl PyStreamPlanner {
    #[new]
    #[pyo3(signature = (budget=None))]
    fn new(budget: Option<PyStreamingBudget>) -> Self {
        Self(pdviewx::StreamPlanner::new(
            budget.map_or_else(pdviewx::StreamingBudget::default, |value| value.0),
        ))
    }
    #[getter]
    fn budget(&self) -> PyStreamingBudget {
        PyStreamingBudget(self.0.budget())
    }
    fn plan(&mut self, requests: Vec<PyChunkRequest>) -> PyStreamPlan {
        let requests = requests.iter().map(|request| request.0).collect::<Vec<_>>();
        PyStreamPlan(self.0.plan(&requests))
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyLodLevel>()?;
    module.add_class::<PyLodPolicy>()?;
    module.add_class::<PyStreamingBudget>()?;
    module.add_class::<PyChunkKey>()?;
    module.add_class::<PyChunkRequest>()?;
    module.add_class::<PyStreamPlan>()?;
    module.add_class::<PyStreamPlanner>()
}
