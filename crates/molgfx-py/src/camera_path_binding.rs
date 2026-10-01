//! Camera paths: keyframed, validated fly-throughs sampled by time.

use crate::authoring_binding::PyCamera;
use molgfx::camera::{CameraEasing, CameraPath};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

/// Cameras at strictly increasing times, interpolated between keyframes.
#[derive(Clone, Debug)]
#[pyclass(name = "CameraPath", frozen, from_py_object)]
pub(super) struct PyCameraPath(pub(super) CameraPath);

fn easing(name: &str) -> PyResult<CameraEasing> {
    match name {
        "linear" => Ok(CameraEasing::Linear),
        "smooth_step" => Ok(CameraEasing::SmoothStep),
        _ => Err(PyValueError::new_err(
            "easing must be 'linear' or 'smooth_step'",
        )),
    }
}

#[pymethods]
impl PyCameraPath {
    /// Raises when fewer than two keyframes, times that do not strictly
    /// increase, mixed projections, or a degenerate camera are given.
    #[new]
    #[pyo3(signature = (keyframes, *, easing="smooth_step"))]
    fn new(keyframes: Vec<(f64, PyRef<'_, PyCamera>)>, easing: &str) -> PyResult<Self> {
        let frames: Vec<(f64, molgfx::Camera)> = keyframes
            .into_iter()
            .map(|(seconds, camera)| (seconds, camera.0))
            .collect();
        molgfx::camera::path(&frames, self::easing(easing)?)
            .map(Self)
            .map_err(|error| PyValueError::new_err(error.to_string()))
    }

    /// The camera at `seconds`, clamped to the path's ends; `None` for a
    /// non-finite time.
    fn sample(&self, seconds: f64) -> Option<PyCamera> {
        self.0.sample(seconds).map(PyCamera)
    }

    /// First and last keyframe time in seconds.
    #[getter]
    fn range(&self) -> (f64, f64) {
        let [start, end] = self.0.range();
        (start, end)
    }

    fn __len__(&self) -> usize {
        self.0.keyframes().len()
    }

    fn __repr__(&self) -> String {
        let [start, end] = self.0.range();
        format!(
            "CameraPath(keyframes={}, range=({start}, {end}))",
            self.0.keyframes().len()
        )
    }
}

pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyCameraPath>()
}
