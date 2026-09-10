//! Python adapters for caller-decoded dynamic covalent topology.

use crate::error::core;
use pyo3::prelude::*;
use std::sync::Arc;

#[pyclass(name = "TopologyBond", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyTopologyBond(pub(crate) pdviewx::TopologyBond);

#[pymethods]
impl PyTopologyBond {
    #[new]
    #[pyo3(signature = (atom_a, atom_b, aromatic=false))]
    fn new(atom_a: u32, atom_b: u32, aromatic: bool) -> PyResult<Self> {
        core(pdviewx::TopologyBond::new(atom_a, atom_b, aromatic)).map(Self)
    }

    #[getter]
    fn atoms(&self) -> (u32, u32) {
        let [a, b] = self.0.atoms();
        (a, b)
    }

    #[getter]
    fn aromatic(&self) -> bool {
        self.0.is_aromatic()
    }
}

#[pyclass(name = "BondTopologyFrame", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyBondTopologyFrame(pub(crate) pdviewx::BondTopologyFrame);

#[pymethods]
impl PyBondTopologyFrame {
    #[new]
    fn new(
        index: u64,
        time_seconds: f32,
        atom_count: u32,
        bonds: Vec<PyTopologyBond>,
        provenance: &str,
    ) -> PyResult<Self> {
        let bonds = bonds.into_iter().map(|bond| bond.0).collect::<Vec<_>>();
        core(pdviewx::BondTopologyFrame::new(
            index,
            time_seconds,
            atom_count,
            Arc::from(bonds.into_boxed_slice()),
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
    fn atom_count(&self) -> u32 {
        self.0.atom_count()
    }

    #[getter]
    fn bonds(&self) -> Vec<PyTopologyBond> {
        self.0.bonds().iter().copied().map(PyTopologyBond).collect()
    }

    #[getter]
    fn provenance(&self) -> String {
        self.0.provenance().to_owned()
    }
}

#[pyclass(name = "BondTopologySegment", from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyBondTopologySegment(pub(crate) pdviewx::BondTopologySegment);

#[pymethods]
impl PyBondTopologySegment {
    #[new]
    fn new(
        start: PyBondTopologyFrame,
        end: PyBondTopologyFrame,
        sample_seconds: f32,
    ) -> PyResult<Self> {
        core(pdviewx::BondTopologySegment::new(
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
    fn start(&self) -> PyBondTopologyFrame {
        PyBondTopologyFrame(self.0.start().clone())
    }

    #[getter]
    fn end(&self) -> PyBondTopologyFrame {
        PyBondTopologyFrame(self.0.end().clone())
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
    fn bonds(&self) -> Vec<(PyTopologyBond, f32)> {
        self.0
            .bonds()
            .map(|active| (PyTopologyBond(active.bond()), active.weight()))
            .collect()
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyTopologyBond>()?;
    module.add_class::<PyBondTopologyFrame>()?;
    module.add_class::<PyBondTopologySegment>()
}
