//! Thin Python adapters for the Rust residency state machine.

use super::dataset::{PyChunkFootprint, PyChunkId, PyDatasetId};
use super::streaming::PyLodLevel;
use crate::error::residency;
use pyo3::prelude::*;

#[pyclass(name = "ResidencyBudget", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyResidencyBudget(pub(crate) molgfx::semantic::ResidencyBudget);

#[pymethods]
impl PyResidencyBudget {
    #[new]
    #[pyo3(signature = (cpu, staging, gpu_hot, gpu_warm, in_flight))]
    fn new(cpu: u64, staging: u64, gpu_hot: u64, gpu_warm: u64, in_flight: u64) -> Self {
        Self(molgfx::semantic::ResidencyBudget {
            cpu,
            staging,
            gpu_hot,
            gpu_warm,
            in_flight,
        })
    }

    #[staticmethod]
    fn default() -> Self {
        Self(molgfx::semantic::ResidencyBudget::default())
    }

    #[getter]
    fn cpu(&self) -> u64 {
        self.0.cpu
    }

    #[getter]
    fn staging(&self) -> u64 {
        self.0.staging
    }

    #[getter]
    fn gpu_hot(&self) -> u64 {
        self.0.gpu_hot
    }

    #[getter]
    fn gpu_warm(&self) -> u64 {
        self.0.gpu_warm
    }

    #[getter]
    fn in_flight(&self) -> u64 {
        self.0.in_flight
    }
}

#[pyclass(name = "ResidencyUsage", frozen, skip_from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyResidencyUsage(pub(crate) molgfx::semantic::Usage);

#[pymethods]
impl PyResidencyUsage {
    #[getter]
    fn cpu(&self) -> u64 {
        self.0.cpu
    }

    #[getter]
    fn staging(&self) -> u64 {
        self.0.staging
    }

    #[getter]
    fn gpu_hot(&self) -> u64 {
        self.0.gpu_hot
    }

    #[getter]
    fn gpu_warm(&self) -> u64 {
        self.0.gpu_warm
    }

    #[getter]
    fn in_flight(&self) -> u64 {
        self.0.in_flight
    }
}

#[pyclass(name = "ResidencyClass", frozen, eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PyResidencyClass {
    Hot,
    Warm,
}

impl From<PyResidencyClass> for molgfx::semantic::ResidencyClass {
    fn from(value: PyResidencyClass) -> Self {
        match value {
            PyResidencyClass::Hot => Self::Hot,
            PyResidencyClass::Warm => Self::Warm,
        }
    }
}

impl From<molgfx::semantic::ResidencyClass> for PyResidencyClass {
    fn from(value: molgfx::semantic::ResidencyClass) -> Self {
        match value {
            molgfx::semantic::ResidencyClass::Hot => Self::Hot,
            molgfx::semantic::ResidencyClass::Warm => Self::Warm,
        }
    }
}

#[pyclass(name = "ResidencyPhase", frozen, eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PyResidencyPhase {
    Absent,
    Requested,
    ReadyCpu,
    Uploading,
    Resident,
}

impl From<molgfx::semantic::ResidencyPhase> for PyResidencyPhase {
    fn from(value: molgfx::semantic::ResidencyPhase) -> Self {
        match value {
            molgfx::semantic::ResidencyPhase::Absent => Self::Absent,
            molgfx::semantic::ResidencyPhase::Requested => Self::Requested,
            molgfx::semantic::ResidencyPhase::ReadyCpu => Self::ReadyCpu,
            molgfx::semantic::ResidencyPhase::Uploading => Self::Uploading,
            molgfx::semantic::ResidencyPhase::Resident => Self::Resident,
        }
    }
}

#[pyclass(name = "ResidencyFailure", frozen, eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PyResidencyFailure {
    Provider,
    InvalidPayload,
    BudgetExceeded,
}

impl From<PyResidencyFailure> for molgfx::semantic::FailureReason {
    fn from(value: PyResidencyFailure) -> Self {
        match value {
            PyResidencyFailure::Provider => Self::Provider,
            PyResidencyFailure::InvalidPayload => Self::InvalidPayload,
            PyResidencyFailure::BudgetExceeded => Self::BudgetExceeded,
        }
    }
}

impl From<molgfx::semantic::FailureReason> for PyResidencyFailure {
    fn from(value: molgfx::semantic::FailureReason) -> Self {
        match value {
            molgfx::semantic::FailureReason::Provider => Self::Provider,
            molgfx::semantic::FailureReason::InvalidPayload => Self::InvalidPayload,
            molgfx::semantic::FailureReason::BudgetExceeded => Self::BudgetExceeded,
        }
    }
}

#[pyclass(name = "ResidencyKey", frozen, eq, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PyResidencyKey(pub(crate) molgfx::semantic::ResidencyKey);

#[pymethods]
impl PyResidencyKey {
    #[new]
    fn new(dataset: PyDatasetId, chunk: PyChunkId, detail: PyLodLevel) -> Self {
        Self(molgfx::semantic::ResidencyKey {
            dataset: dataset.0,
            chunk: chunk.0,
            detail: detail.into(),
        })
    }

    #[getter]
    fn dataset(&self) -> PyDatasetId {
        PyDatasetId(self.0.dataset)
    }

    #[getter]
    fn chunk(&self) -> PyChunkId {
        PyChunkId(self.0.chunk)
    }

    #[getter]
    fn detail(&self) -> PyLodLevel {
        self.0.detail.into()
    }
}

#[pyclass(name = "ResidencyTicket", frozen, eq, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PyResidencyTicket(pub(crate) molgfx::semantic::ResidencyTicket);

#[pymethods]
impl PyResidencyTicket {
    #[getter]
    fn key(&self) -> PyResidencyKey {
        PyResidencyKey(self.0.key)
    }

    #[getter]
    fn generation(&self) -> u64 {
        self.0.generation()
    }
}

#[pyclass(name = "ResidencyRequest", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyResidencyRequest(pub(crate) molgfx::semantic::ResidencyRequest);

#[pymethods]
impl PyResidencyRequest {
    #[new]
    fn new(
        key: PyResidencyKey,
        footprint: PyChunkFootprint,
        residency_class: PyResidencyClass,
        priority: i32,
    ) -> Self {
        Self(molgfx::semantic::ResidencyRequest {
            key: key.0,
            footprint: footprint.0,
            class: residency_class.into(),
            priority,
        })
    }

    #[getter]
    fn key(&self) -> PyResidencyKey {
        PyResidencyKey(self.0.key)
    }

    #[getter]
    fn footprint(&self) -> PyChunkFootprint {
        PyChunkFootprint(self.0.footprint)
    }

    #[getter]
    fn residency_class(&self) -> PyResidencyClass {
        self.0.class.into()
    }

    #[getter]
    fn priority(&self) -> i32 {
        self.0.priority
    }
}

#[pyclass(name = "ResidencySnapshot", frozen, skip_from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyResidencySnapshot(pub(crate) molgfx::semantic::ResidencySnapshot);

#[pymethods]
impl PyResidencySnapshot {
    #[getter]
    fn phase(&self) -> PyResidencyPhase {
        self.0.phase.into()
    }

    #[getter]
    fn generation(&self) -> u64 {
        self.0.generation
    }

    #[getter]
    fn failure(&self) -> Option<PyResidencyFailure> {
        self.0.failure.map(Into::into)
    }
}

#[pyclass(name = "ResidencyEviction", frozen, skip_from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyResidencyEviction(pub(crate) molgfx::semantic::Eviction);

#[pymethods]
impl PyResidencyEviction {
    #[getter]
    fn key(&self) -> PyResidencyKey {
        PyResidencyKey(self.0.key)
    }

    #[getter]
    fn generation(&self) -> u64 {
        self.0.generation
    }
}

#[pyclass(name = "StaleCompletion", frozen, skip_from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyStaleCompletion(pub(crate) molgfx::semantic::StaleCompletion);

#[pymethods]
impl PyStaleCompletion {
    #[getter]
    fn ticket(&self) -> PyResidencyTicket {
        PyResidencyTicket(self.0.ticket)
    }

    #[getter]
    fn current_generation(&self) -> u64 {
        self.0.current_generation
    }

    #[getter]
    fn current_phase(&self) -> PyResidencyPhase {
        self.0.current_phase.into()
    }
}

#[pyclass(name = "DeviceLossReport", frozen, skip_from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyDeviceLossReport(pub(crate) molgfx::semantic::DeviceLossReport);

#[pymethods]
impl PyDeviceLossReport {
    #[getter]
    fn invalidated(&self) -> usize {
        self.0.invalidated
    }

    #[getter]
    fn ready_cpu(&self) -> usize {
        self.0.ready_cpu
    }
}

#[pyclass(name = "ResidencyOutput", skip_from_py_object)]
#[derive(Clone, Debug, Default)]
pub(crate) struct PyResidencyOutput(pub(crate) molgfx::semantic::ResidencyOutput);

#[pymethods]
impl PyResidencyOutput {
    #[new]
    fn new() -> Self {
        Self::default()
    }

    #[getter]
    fn requests(&self) -> Vec<PyResidencyTicket> {
        self.0
            .requests
            .iter()
            .copied()
            .map(PyResidencyTicket)
            .collect()
    }

    #[getter]
    fn cancellations(&self) -> Vec<PyResidencyTicket> {
        self.0
            .cancellations
            .iter()
            .copied()
            .map(PyResidencyTicket)
            .collect()
    }

    #[getter]
    fn ready_uploads(&self) -> Vec<PyResidencyTicket> {
        self.0
            .ready_uploads
            .iter()
            .copied()
            .map(PyResidencyTicket)
            .collect()
    }

    #[getter]
    fn evictions(&self) -> Vec<PyResidencyEviction> {
        self.0
            .evictions
            .iter()
            .copied()
            .map(PyResidencyEviction)
            .collect()
    }

    #[getter]
    fn stale(&self) -> Vec<PyStaleCompletion> {
        self.0
            .stale
            .iter()
            .copied()
            .map(PyStaleCompletion)
            .collect()
    }
}

#[pyclass(name = "ResidencyMachine")]
#[derive(Debug)]
pub(crate) struct PyResidencyMachine(pub(crate) molgfx::semantic::ResidencyMachine);

#[pymethods]
impl PyResidencyMachine {
    #[new]
    #[pyo3(signature = (budget=None))]
    fn new(budget: Option<PyResidencyBudget>) -> Self {
        let budget = match budget {
            Some(value) => value.0,
            None => molgfx::semantic::ResidencyBudget::default(),
        };
        Self(molgfx::semantic::ResidencyMachine::new(budget))
    }

    #[getter]
    fn budget(&self) -> PyResidencyBudget {
        PyResidencyBudget(self.0.budget())
    }

    #[getter]
    fn usage(&self) -> PyResidencyUsage {
        PyResidencyUsage(self.0.usage())
    }

    fn snapshot(&self, key: PyResidencyKey) -> PyResidencySnapshot {
        PyResidencySnapshot(self.0.snapshot(key.0))
    }

    fn request_into(
        &mut self,
        request: PyResidencyRequest,
        mut output: PyRefMut<'_, PyResidencyOutput>,
    ) -> PyResult<PyResidencyTicket> {
        residency(self.0.request_into(request.0, &mut output.0)).map(PyResidencyTicket)
    }

    fn ready_cpu_into(
        &mut self,
        ticket: PyResidencyTicket,
        mut output: PyRefMut<'_, PyResidencyOutput>,
    ) -> PyResult<bool> {
        residency(self.0.ready_cpu_into(ticket.0, &mut output.0))
    }

    fn begin_upload_into(
        &mut self,
        ticket: PyResidencyTicket,
        mut output: PyRefMut<'_, PyResidencyOutput>,
    ) -> PyResult<bool> {
        residency(self.0.begin_upload_into(ticket.0, &mut output.0))
    }

    fn complete_upload_into(
        &mut self,
        ticket: PyResidencyTicket,
        mut output: PyRefMut<'_, PyResidencyOutput>,
    ) -> PyResult<bool> {
        residency(self.0.complete_upload_into(ticket.0, &mut output.0))
    }

    fn cancel_into(
        &mut self,
        ticket: PyResidencyTicket,
        mut output: PyRefMut<'_, PyResidencyOutput>,
    ) -> bool {
        self.0.cancel_into(ticket.0, &mut output.0)
    }

    fn fail_into(
        &mut self,
        ticket: PyResidencyTicket,
        reason: PyResidencyFailure,
        mut output: PyRefMut<'_, PyResidencyOutput>,
    ) -> bool {
        self.0.fail_into(ticket.0, reason.into(), &mut output.0)
    }

    fn set_budget_into(
        &mut self,
        budget: PyResidencyBudget,
        mut output: PyRefMut<'_, PyResidencyOutput>,
    ) {
        self.0.set_budget_into(budget.0, &mut output.0);
    }

    fn device_lost_into(
        &mut self,
        mut output: PyRefMut<'_, PyResidencyOutput>,
    ) -> PyResult<PyDeviceLossReport> {
        residency(self.0.device_lost_into(&mut output.0)).map(PyDeviceLossReport)
    }
}
