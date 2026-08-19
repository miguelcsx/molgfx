//! Python adapters for topology-stable trajectory interpolation.

use crate::error::core;
use crate::math::{PyAabb, PyVec3};
use pyo3::prelude::*;
use std::sync::Arc;

#[pyclass(name = "TrajectoryFrame", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyTrajectoryFrame(pub(crate) pdviewx::TrajectoryFrame);

#[pymethods]
impl PyTrajectoryFrame {
    #[new]
    fn new(
        index: u64,
        time_seconds: f32,
        positions: Vec<PyVec3>,
        provenance: &str,
    ) -> PyResult<Self> {
        let positions: Vec<[f32; 3]> = positions
            .into_iter()
            .map(|position| position.0.to_array())
            .collect();
        core(pdviewx::TrajectoryFrame::new(
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

    #[getter]
    fn positions(&self) -> Vec<PyVec3> {
        self.0
            .positions()
            .iter()
            .copied()
            .map(pdviewx::Vec3::from_array)
            .map(crate::math::PyVec3)
            .collect()
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
pub(crate) struct PyTrajectorySegment(pub(crate) pdviewx::TrajectorySegment);

#[pymethods]
impl PyTrajectorySegment {
    #[new]
    fn new(
        start: PyTrajectoryFrame,
        end: PyTrajectoryFrame,
        sample_seconds: f32,
    ) -> PyResult<Self> {
        core(pdviewx::TrajectorySegment::new(
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

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyTrajectoryFrame>()?;
    module.add_class::<PyTrajectorySegment>()
}
