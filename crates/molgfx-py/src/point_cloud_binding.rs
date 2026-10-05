//! Compact `NumPy` coordinate groups crossing the Python boundary once.

use crate::binding::error;
use numpy::{PyReadonlyArray2, PyUntypedArrayMethods};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use std::sync::Arc;

#[pyclass(name = "PointCloud", frozen)]
pub(super) struct PyPointCloud(pub(super) molgfx::PointCloud);

#[pymethods]
impl PyPointCloud {
    /// Retains colour groups of float32 (N, 3) arrays as compact native positions.
    ///
    /// Input is copied once; later array mutation cannot change the cloud. Empty
    /// groups are rejected. This native off-screen scene has no browser transport.
    #[new]
    #[pyo3(signature = (groups, *, radius=0.04))]
    fn new(groups: Vec<(PyReadonlyArray2<'_, f32>, [u8; 3])>, radius: f32) -> PyResult<Self> {
        let mut native = Vec::with_capacity(groups.len());
        for (positions, [red, green, blue]) in groups {
            if positions.shape()[1] != 3 {
                return Err(PyValueError::new_err(
                    "point coordinates require shape (N, 3)",
                ));
            }
            let rows: Arc<[[f32; 3]]> = positions
                .as_array()
                .rows()
                .into_iter()
                .map(|row| [row[0], row[1], row[2]])
                .collect();
            native.push((rows, molgfx::Color::rgb(red, green, blue)));
        }
        molgfx::PointCloud::new(native, radius)
            .map(Self)
            .map_err(error)
    }

    #[getter]
    fn point_count(&self) -> usize {
        self.0.point_count()
    }

    /// Composes points with an immutable snapshot of a molecular scene.
    fn with_scene(&self, scene: &crate::scene_binding::PyScene) -> PyResult<Self> {
        self.0.with_scene(&scene.inner).map(Self).map_err(error)
    }
}

pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyPointCloud>()
}
