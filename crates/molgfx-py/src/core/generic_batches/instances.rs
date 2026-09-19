//! Rigid-instance batches: one shared template, one style, many transforms.

use super::super::{PyInstanceBatchHandle, PyScene};
use super::arrays::{contiguous, ordered_rows, validate_columns};
use super::template::PyAnalyticTemplate;
use crate::error::{core, value};
use crate::math::{PyAabb, PyQuat, PyRgba8, PyVec3};
use numpy::{
    PyArray1, PyArray2, PyArrayMethods, PyReadonlyArray1, PyReadonlyArray2, PyUntypedArrayMethods,
};
use pyo3::prelude::*;
use std::sync::Arc;

/// Batch-wide fallback used when no visual program targets the batch.
#[pyclass(name = "InstanceStyle", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyInstanceStyle(pub(crate) molgfx::core::InstanceStyle);

#[pymethods]
impl PyInstanceStyle {
    #[new]
    #[pyo3(signature = (color=None))]
    fn new(color: Option<PyRgba8>) -> Self {
        let fallback = molgfx::core::InstanceStyle::default();
        Self(molgfx::core::InstanceStyle {
            color: match color {
                Some(color) => color.0,
                None => fallback.color,
            },
        })
    }

    #[getter]
    fn color(&self) -> PyRgba8 {
        self.0.color.into()
    }

    fn __repr__(&self) -> String {
        format!("InstanceStyle(color={:?})", self.0.color)
    }
}

/// One validated placement of a shared template.
#[pyclass(name = "RigidInstance", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyRigidInstance(pub(crate) molgfx::core::RigidInstance);

#[pymethods]
impl PyRigidInstance {
    #[new]
    fn new(translation: PyVec3, orientation: PyQuat, scale: f32) -> PyResult<Self> {
        core(molgfx::core::RigidInstance::new(
            translation.0,
            orientation.0,
            scale,
        ))
        .map(Self)
    }

    #[getter]
    fn translation(&self) -> PyVec3 {
        PyVec3(self.0.translation())
    }

    #[getter]
    fn orientation(&self) -> PyQuat {
        PyQuat(molgfx::math::Quat::from_array(self.0.orientation))
    }

    #[getter]
    fn scale(&self) -> f32 {
        self.0.scale()
    }

    fn __repr__(&self) -> String {
        format!(
            "RigidInstance(translation={:?}, scale={})",
            self.0.translation(),
            self.0.scale()
        )
    }
}

/// One inserted instance batch as the scene holds it.
#[pyclass(name = "InstanceBatch", frozen, skip_from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyInstanceBatch(pub(crate) molgfx::core::InstanceBatch);

impl From<&molgfx::core::InstanceBatch> for PyInstanceBatch {
    fn from(value: &molgfx::core::InstanceBatch) -> Self {
        Self(value.clone())
    }
}

#[pymethods]
impl PyInstanceBatch {
    #[getter]
    fn template(&self) -> PyAnalyticTemplate {
        PyAnalyticTemplate(Arc::clone(self.0.template()))
    }

    #[getter]
    fn style(&self) -> PyInstanceStyle {
        PyInstanceStyle(self.0.style())
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
    fn instance_count(&self) -> usize {
        self.0.transforms().len()
    }

    #[getter]
    fn row_count(&self) -> u32 {
        self.0.source_rows().len()
    }

    /// Copies one row per instance as `translation xyz, scale, orientation xyzw`.
    fn copy_transforms_numpy<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyArray2<f32>>> {
        let rows = self.0.transforms().len();
        let mut columns = Vec::with_capacity(rows * 8);
        for instance in self.0.transforms().iter() {
            let translation = instance.translation();
            columns.extend_from_slice(&[translation.x, translation.y, translation.z]);
            columns.push(instance.scale());
            columns.extend_from_slice(&instance.orientation);
        }
        PyArray1::from_vec(py, columns).reshape((rows, 8))
    }

    fn __repr__(&self) -> String {
        format!(
            "InstanceBatch(instance_count={}, row_count={})",
            self.0.transforms().len(),
            self.0.source_rows().len()
        )
    }
}

#[pymethods]
impl PyScene {
    #[pyo3(signature = (template, namespace, translations, orientations, scales, style=None))]
    fn add_instance_batch_from_numpy(
        &mut self,
        template: PyRef<'_, PyAnalyticTemplate>,
        namespace: u64,
        translations: PyReadonlyArray2<'_, f32>,
        orientations: PyReadonlyArray2<'_, f32>,
        scales: PyReadonlyArray1<'_, f32>,
        style: Option<PyInstanceStyle>,
    ) -> PyResult<PyInstanceBatchHandle> {
        validate_columns("translations", translations.shape(), 3)?;
        validate_columns("orientations", orientations.shape(), 4)?;
        let count = translations.shape()[0];
        if orientations.shape()[0] != count || scales.shape() != [count] {
            return Err(value(
                "instance transform columns must have the same row count",
            ));
        }
        let translations = contiguous("translations", &translations)?;
        let orientations = contiguous("orientations", &orientations)?;
        let scales = scales
            .as_slice()
            .map_err(|_| value("scales must be C-contiguous float32"))?;
        let mut transforms = Vec::with_capacity(count);
        for ((translation, orientation), scale) in translations
            .chunks_exact(3)
            .zip(orientations.chunks_exact(4))
            .zip(scales)
        {
            transforms.push(core(molgfx::core::RigidInstance::new(
                molgfx::math::Vec3::new(translation[0], translation[1], translation[2]),
                molgfx::math::Quat::from_array([
                    orientation[0],
                    orientation[1],
                    orientation[2],
                    orientation[3],
                ]),
                *scale,
            ))?);
        }
        let rows = ordered_rows(namespace, count)?;
        let style = match style {
            Some(style) => style.0,
            None => molgfx::core::InstanceStyle::default(),
        };
        core(molgfx::core::InstanceBatch::new(
            template.native(),
            Arc::from(transforms),
            rows,
        ))
        .map(|batch| batch.with_style(style))
        .map(|batch| self.inner.add_instance_batch(batch).into())
    }

    /// Resolves an instance batch by generational handle.
    fn instance_batch(&self, handle: PyInstanceBatchHandle) -> Option<PyInstanceBatch> {
        self.inner.instance_batch(handle.0).map(Into::into)
    }

    /// Iterates instance batches in stable slot order.
    fn instance_batches(&self) -> Vec<(PyInstanceBatchHandle, PyInstanceBatch)> {
        self.inner
            .instance_batches()
            .map(|(handle, batch)| (handle.into(), batch.into()))
            .collect()
    }

    /// Removes one instance batch and invalidates instance and part rows.
    fn remove_instance_batch(&mut self, handle: PyInstanceBatchHandle) -> Option<PyInstanceBatch> {
        self.inner
            .remove_instance_batch(handle.0)
            .map(PyInstanceBatch)
    }
}
