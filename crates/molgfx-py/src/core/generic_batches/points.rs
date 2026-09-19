//! Point batches: the shared glyph, the batch style and the read-back record.

use super::super::{PyPointBatchHandle, PyScene};
use super::arrays::{ordered_rows, vec3_rows};
use crate::error::core;
use crate::math::{PyAabb, PyRgba8};
use numpy::{PyArray1, PyArray2, PyArrayMethods, PyReadonlyArray2};
use pyo3::prelude::*;
use std::sync::Arc;

/// The one analytic glyph every point in a batch draws.
#[pyclass(name = "PointGlyph", frozen, eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PyPointGlyph {
    Disc,
    Sphere,
}

impl From<PyPointGlyph> for molgfx::core::PointGlyph {
    fn from(value: PyPointGlyph) -> Self {
        match value {
            PyPointGlyph::Disc => Self::Disc,
            PyPointGlyph::Sphere => Self::Sphere,
        }
    }
}

impl From<molgfx::core::PointGlyph> for PyPointGlyph {
    fn from(value: molgfx::core::PointGlyph) -> Self {
        match value {
            molgfx::core::PointGlyph::Disc => Self::Disc,
            molgfx::core::PointGlyph::Sphere => Self::Sphere,
        }
    }
}

/// Batch-wide fallback appearance. Per-row variation belongs in attributes.
#[pyclass(name = "PointStyle", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyPointStyle(pub(crate) molgfx::core::PointStyle);

#[pymethods]
impl PyPointStyle {
    #[new]
    #[pyo3(signature = (radius=0.1, color=None))]
    fn new(radius: f32, color: Option<PyRgba8>) -> Self {
        let fallback = molgfx::core::PointStyle::default();
        Self(molgfx::core::PointStyle {
            radius,
            color: match color {
                Some(color) => color.0,
                None => fallback.color,
            },
        })
    }

    #[getter]
    fn radius(&self) -> f32 {
        self.0.radius
    }

    #[getter]
    fn color(&self) -> PyRgba8 {
        self.0.color.into()
    }

    fn __repr__(&self) -> String {
        format!(
            "PointStyle(radius={}, color={:?})",
            self.0.radius, self.0.color
        )
    }
}

/// One inserted point batch as the scene holds it.
///
/// The bulk position column stays in the scene; a caller that needs it reads
/// it back through [`PyPointBatch::copy_positions_numpy`].
#[pyclass(name = "PointBatch", frozen, skip_from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyPointBatch(pub(crate) molgfx::core::PointBatch);

impl From<&molgfx::core::PointBatch> for PyPointBatch {
    fn from(value: &molgfx::core::PointBatch) -> Self {
        Self(value.clone())
    }
}

#[pymethods]
impl PyPointBatch {
    #[getter]
    fn glyph(&self) -> PyPointGlyph {
        self.0.glyph().into()
    }

    #[getter]
    fn style(&self) -> PyPointStyle {
        PyPointStyle(self.0.style())
    }

    #[getter]
    fn bounds(&self) -> PyAabb {
        PyAabb(self.0.bounds())
    }

    #[getter]
    fn visible(&self) -> bool {
        self.0.visible()
    }

    #[getter]
    fn row_count(&self) -> u32 {
        self.0.source_rows().len()
    }

    /// Copies the shared 12-byte rows into one `(row_count, 3)` table.
    fn copy_positions_numpy<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyArray2<f32>>> {
        let rows = self.0.positions().len();
        let flat = PyArray1::from_slice(py, bytemuck::cast_slice(self.0.positions()));
        flat.reshape((rows, 3))
    }

    fn __repr__(&self) -> String {
        format!(
            "PointBatch(glyph={:?}, row_count={})",
            self.0.glyph(),
            self.0.source_rows().len()
        )
    }
}

#[pymethods]
impl PyScene {
    #[pyo3(signature = (namespace, positions, style=None, glyph=PyPointGlyph::Disc))]
    fn add_point_batch_from_numpy(
        &mut self,
        namespace: u64,
        positions: PyReadonlyArray2<'_, f32>,
        style: Option<PyPointStyle>,
        glyph: PyPointGlyph,
    ) -> PyResult<PyPointBatchHandle> {
        let positions = vec3_rows("positions", &positions)?;
        let rows = ordered_rows(namespace, positions.len())?;
        let style = match style {
            Some(style) => style.0,
            None => molgfx::core::PointStyle::default(),
        };
        core(molgfx::core::PointBatch::new(
            Arc::from(positions),
            rows,
            glyph.into(),
            style,
        ))
        .map(|batch| self.inner.add_point_batch(batch).into())
    }

    /// Resolves a point batch by generational handle.
    fn point_batch(&self, handle: PyPointBatchHandle) -> Option<PyPointBatch> {
        self.inner.point_batch(handle.0).map(Into::into)
    }

    /// Iterates point batches in stable slot order.
    fn point_batches(&self) -> Vec<(PyPointBatchHandle, PyPointBatch)> {
        self.inner
            .point_batches()
            .map(|(handle, batch)| (handle.into(), batch.into()))
            .collect()
    }

    /// Removes one point batch and invalidates its handle.
    fn remove_point_batch(&mut self, handle: PyPointBatchHandle) -> Option<PyPointBatch> {
        self.inner.remove_point_batch(handle.0).map(PyPointBatch)
    }
}
