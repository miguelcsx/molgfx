//! Python context manager for one atomic semantic scene transaction.

use crate::binding::{PyScene, error};
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyModule};

#[pyclass(name = "SceneTransaction")]
pub(super) struct PySceneTransaction {
    scene: Py<PyScene>,
    closed: bool,
}

#[pymethods]
impl PySceneTransaction {
    fn __enter__(&self, py: Python<'_>) -> Py<PyScene> {
        self.scene.clone_ref(py)
    }

    fn __exit__(
        &mut self,
        py: Python<'_>,
        exception_type: Option<&Bound<'_, PyAny>>,
        _exception: Option<&Bound<'_, PyAny>>,
        _traceback: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<bool> {
        if self.closed {
            return Err(PyRuntimeError::new_err(
                "scene transaction is already closed",
            ));
        }
        self.closed = true;
        let mut scene = self.scene.borrow_mut(py);
        let operations = scene
            .pending
            .take()
            .ok_or_else(|| PyRuntimeError::new_err("scene transaction is not active"))?;
        if exception_type.is_some() {
            return Ok(false);
        }
        let base_revision = scene.inner.revision();
        scene
            .inner
            .apply(&molgfx::ScenePatch {
                base_revision,
                operations,
            })
            .map_err(error)?;
        Ok(false)
    }
}

#[pymethods]
impl PyScene {
    fn transaction(slf: Py<Self>, py: Python<'_>) -> PyResult<PySceneTransaction> {
        {
            let mut scene = slf.borrow_mut(py);
            if scene.pending.is_some() {
                return Err(PyValueError::new_err("scene transactions cannot be nested"));
            }
            scene.pending = Some(Vec::new());
        }
        Ok(PySceneTransaction {
            scene: slf,
            closed: false,
        })
    }
}

pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PySceneTransaction>()
}
