//! Python adapters for topology-stable trajectory interpolation.

use crate::core::{PyScene, PyStructureHandle};
use crate::error::core;
use crate::math::PyAabb;
use numpy::{PyArray1, PyArray2, PyArrayMethods, PyReadonlyArray2, PyUntypedArrayMethods};
use pyo3::prelude::*;
use std::sync::Arc;

#[pyclass(name = "TrajectoryChunkWindow", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyTrajectoryChunkWindow(pub(crate) molgfx::render::TrajectoryChunkWindow);

#[pymethods]
impl PyTrajectoryChunkWindow {
    #[new]
    fn new(
        structure: crate::semantic::PyResidencyTicket,
        start: crate::semantic::PyResidencyTicket,
        end: crate::semantic::PyResidencyTicket,
        interpolation: f32,
    ) -> PyResult<Self> {
        molgfx::render::TrajectoryChunkWindow::new(structure.0, start.0, end.0, interpolation)
            .map(Self)
            .map_err(|error| crate::error::value(error.to_string()))
    }

    #[getter]
    fn structure(&self) -> crate::semantic::PyResidencyTicket {
        crate::semantic::PyResidencyTicket(self.0.structure)
    }

    #[getter]
    fn start(&self) -> crate::semantic::PyResidencyTicket {
        crate::semantic::PyResidencyTicket(self.0.start)
    }

    #[getter]
    fn end(&self) -> crate::semantic::PyResidencyTicket {
        crate::semantic::PyResidencyTicket(self.0.end)
    }

    #[getter]
    fn interpolation(&self) -> f32 {
        self.0.interpolation
    }
}

#[pyclass(name = "TrajectoryFrame", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyTrajectoryFrame(pub(crate) molgfx::core::TrajectoryFrame);

#[pymethods]
impl PyTrajectoryFrame {
    /// Copies one explicit C-contiguous `(N, 3)` float32 frame into Rust-owned
    /// storage. The name makes the ownership transition visible at the callsite.
    #[staticmethod]
    fn copy_from_numpy(
        index: u64,
        time_seconds: f32,
        positions: PyReadonlyArray2<'_, f32>,
        provenance: &str,
    ) -> PyResult<Self> {
        let shape = positions.shape();
        if shape.len() != 2 || shape[1] != 3 {
            return Err(crate::error::value("positions must have shape (N, 3)"));
        }
        let values = positions
            .as_slice()
            .map_err(|_| crate::error::value("positions must be C-contiguous float32"))?;
        let positions = values
            .chunks_exact(3)
            .map(|row| [row[0], row[1], row[2]])
            .collect::<Vec<_>>();
        core(molgfx::core::TrajectoryFrame::new(
            index,
            time_seconds,
            Arc::from(positions.into_boxed_slice()),
            provenance,
        ))
        .map(Self)
    }

    #[getter]
    fn index(&self) -> u64 {
        self.0.index()
    }

    #[getter]
    fn time_seconds(&self) -> f32 {
        self.0.time_seconds()
    }

    fn copy_positions_numpy<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyArray2<f32>>> {
        let rows = self.0.positions().len();
        let flat = PyArray1::from_slice(py, bytemuck::cast_slice(self.0.positions()));
        let result = flat.reshape((rows, 3))?;
        result.readwrite().make_nonwriteable();
        Ok(result)
    }

    #[getter]
    fn provenance(&self) -> String {
        self.0.provenance().to_owned()
    }

    fn aabb(&self) -> PyAabb {
        PyAabb(self.0.aabb())
    }
}

#[pyclass(name = "TrajectorySegment", from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyTrajectorySegment(pub(crate) molgfx::core::TrajectorySegment);

#[pymethods]
impl PyTrajectorySegment {
    #[new]
    fn new(
        start: PyTrajectoryFrame,
        end: PyTrajectoryFrame,
        sample_seconds: f32,
    ) -> PyResult<Self> {
        core(molgfx::core::TrajectorySegment::new(
            start.0,
            end.0,
            sample_seconds,
        ))
        .map(Self)
    }

    fn set_sample_time(&mut self, sample_seconds: f32) -> PyResult<()> {
        core(self.0.set_sample_time(sample_seconds))
    }

    #[getter]
    fn start(&self) -> PyTrajectoryFrame {
        PyTrajectoryFrame(self.0.start().clone())
    }

    #[getter]
    fn end(&self) -> PyTrajectoryFrame {
        PyTrajectoryFrame(self.0.end().clone())
    }

    #[getter]
    fn sample_seconds(&self) -> f32 {
        self.0.sample_seconds()
    }

    #[getter]
    fn interpolation(&self) -> f32 {
        self.0.interpolation()
    }

    #[getter]
    fn atom_count(&self) -> usize {
        self.0.atom_count()
    }

    fn union_aabb(&self) -> PyAabb {
        PyAabb(self.0.union_aabb())
    }
}

#[pyclass(name = "TrajectoryBranch", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyTrajectoryBranch(molgfx::core::TrajectoryBranch);

#[pymethods]
impl PyTrajectoryBranch {
    #[new]
    #[pyo3(signature = (from_state, to_state, event, probability=1.0))]
    fn new(from_state: u32, to_state: u32, event: String, probability: f32) -> PyResult<Self> {
        core(molgfx::core::TrajectoryBranch::new(
            from_state,
            to_state,
            event,
            probability,
        ))
        .map(Self)
    }

    #[getter]
    #[pyo3(name = "from_state")]
    fn state_from(&self) -> u32 {
        self.0.from()
    }

    #[getter]
    fn to_state(&self) -> u32 {
        self.0.to()
    }

    #[getter]
    fn event(&self) -> &str {
        self.0.event()
    }

    #[getter]
    fn probability(&self) -> f32 {
        self.0.probability()
    }
}

#[pyclass(name = "TrajectoryStateGraph")]
#[derive(Debug)]
pub(crate) struct PyTrajectoryStateGraph(molgfx::core::TrajectoryStateGraph);

#[pymethods]
impl PyTrajectoryStateGraph {
    #[new]
    fn new(states: Vec<u32>, branches: Vec<PyTrajectoryBranch>, initial: u32) -> PyResult<Self> {
        core(molgfx::core::TrajectoryStateGraph::new(
            states,
            branches.into_iter().map(|branch| branch.0).collect(),
            initial,
        ))
        .map(Self)
    }

    fn target(&self, event: &str) -> Option<u32> {
        self.0.target(event)
    }

    fn transition(
        &mut self,
        mut scene: PyRefMut<'_, PyScene>,
        structure: PyStructureHandle,
        event: &str,
        segment: PyTrajectorySegment,
    ) -> PyResult<u32> {
        core(
            self.0
                .transition(&mut scene.inner, structure.0, event, segment.0),
        )
    }

    fn seek(
        &mut self,
        mut scene: PyRefMut<'_, PyScene>,
        structure: PyStructureHandle,
        state: u32,
        segment: PyTrajectorySegment,
    ) -> PyResult<()> {
        core(self.0.seek(&mut scene.inner, structure.0, state, segment.0))
    }

    #[getter]
    fn active(&self) -> u32 {
        self.0.active()
    }

    #[getter]
    fn states<'py>(&self, py: Python<'py>) -> Bound<'py, PyArray1<u32>> {
        PyArray1::from_slice(py, self.0.states())
    }

    #[getter]
    fn branches(&self) -> Vec<PyTrajectoryBranch> {
        self.0
            .branches()
            .iter()
            .cloned()
            .map(PyTrajectoryBranch)
            .collect()
    }
}
