//! Python adapters for bounded ordered image sequences.

use super::engine::{PyEngine, PyImage};
use crate::core::PyScene;
use crate::error::{render, value};
use crate::math::PyCamera;
use pyo3::prelude::*;

#[pyclass(name = "SequenceFrame")]
#[derive(Debug)]
pub(crate) struct PySequenceFrame {
    ticket: super::PyFrameTicket,
    image: Option<PyImage>,
}

impl From<pdviewx::SequenceFrame> for PySequenceFrame {
    fn from(value: pdviewx::SequenceFrame) -> Self {
        Self {
            ticket: super::PyFrameTicket(value.ticket),
            image: Some(value.image.into()),
        }
    }
}

#[pymethods]
impl PySequenceFrame {
    #[getter]
    fn ticket(&self) -> super::PyFrameTicket {
        self.ticket
    }

    fn take_image(&mut self) -> PyResult<PyImage> {
        self.image
            .take()
            .ok_or_else(|| value("sequence image has already been transferred"))
    }
}

#[pyclass(name = "SequenceRenderer")]
#[derive(Debug)]
pub(crate) struct PySequenceRenderer {
    inner: Option<pdviewx::SequenceRenderer>,
}

pub(super) fn create(
    engine: &pdviewx::Engine,
    config: super::PySequenceConfig,
) -> PyResult<PySequenceRenderer> {
    render(engine.sequence(config.0)).map(|inner| PySequenceRenderer { inner: Some(inner) })
}

#[pymethods]
impl PySequenceRenderer {
    fn submit(
        &mut self,
        py: Python<'_>,
        engine: &mut PyEngine,
        scene: &PyScene,
        camera: PyCamera,
        timestamp: u64,
    ) -> PyResult<super::PyFrameTicket> {
        let sequence = self
            .inner
            .as_mut()
            .ok_or_else(|| value("sequence has already been finished"))?;
        render(
            py.detach(|| {
                sequence.submit(&mut engine.inner, &scene.inner, &camera.inner, timestamp)
            }),
        )
        .map(super::PyFrameTicket)
    }

    fn poll(&mut self, py: Python<'_>, engine: &mut PyEngine) -> PyResult<Option<PySequenceFrame>> {
        let sequence = self
            .inner
            .as_mut()
            .ok_or_else(|| value("sequence has already been finished"))?;
        render(py.detach(|| sequence.poll(&mut engine.inner))).map(|frame| frame.map(Into::into))
    }

    fn finish(&mut self, py: Python<'_>, engine: &mut PyEngine) -> PyResult<Vec<PySequenceFrame>> {
        let sequence = self
            .inner
            .take()
            .ok_or_else(|| value("sequence has already been finished"))?;
        render(py.detach(|| sequence.finish(&mut engine.inner)))
            .map(|frames| frames.into_iter().map(Into::into).collect())
    }

    #[getter]
    fn pending(&self) -> usize {
        self.inner
            .as_ref()
            .map_or(0, pdviewx::SequenceRenderer::pending)
    }
}

pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PySequenceFrame>()?;
    module.add_class::<PySequenceRenderer>()
}
