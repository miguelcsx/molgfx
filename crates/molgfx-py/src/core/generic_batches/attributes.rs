//! Domain visual state, native-width attributes and introspection.

use super::super::{PyAttributeHandle, PyScene};
use super::{
    PyRowDomain,
    arrays::{validate_columns, vec3_rows},
};
use crate::{
    error::{core, value},
    visual::PyVisualStyle,
};
use numpy::{
    PyArray1, PyArray2, PyArrayMethods, PyReadonlyArray1, PyReadonlyArray2, PyUntypedArrayMethods,
};
use pyo3::prelude::*;
use std::sync::Arc;

/// Physical kind of one typed column.
#[pyclass(name = "AttributeKind", frozen, eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PyAttributeKind {
    Scalar,
    Category,
    Vector,
    Color,
}

impl From<molgfx::core::AttributeKind> for PyAttributeKind {
    fn from(value: molgfx::core::AttributeKind) -> Self {
        match value {
            molgfx::core::AttributeKind::Scalar => Self::Scalar,
            molgfx::core::AttributeKind::Category => Self::Category,
            molgfx::core::AttributeKind::Vector => Self::Vector,
            molgfx::core::AttributeKind::Color => Self::Color,
        }
    }
}

impl From<PyAttributeKind> for molgfx::core::AttributeKind {
    fn from(value: PyAttributeKind) -> Self {
        match value {
            PyAttributeKind::Scalar => Self::Scalar,
            PyAttributeKind::Category => Self::Category,
            PyAttributeKind::Vector => Self::Vector,
            PyAttributeKind::Color => Self::Color,
        }
    }
}

#[pymethods]
impl PyAttributeKind {
    /// Native byte stride retained in host and device storage.
    #[getter]
    fn stride(&self) -> u32 {
        molgfx::core::AttributeKind::from(*self).stride()
    }
}

/// Domain-independent introspection and reproducibility metadata.
#[pyclass(name = "AttributeDescriptor", frozen, skip_from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyAttributeDescriptor(pub(crate) molgfx::core::AttributeDescriptor);

impl From<&molgfx::core::AttributeDescriptor> for PyAttributeDescriptor {
    fn from(value: &molgfx::core::AttributeDescriptor) -> Self {
        Self(value.clone())
    }
}

#[pymethods]
impl PyAttributeDescriptor {
    /// Caller-facing column name.
    #[getter]
    fn name(&self) -> String {
        self.0.name().to_owned()
    }

    /// Declared measured quantity, when the caller named one.
    #[getter]
    fn quantity(&self) -> Option<String> {
        self.0.quantity().map(ToOwned::to_owned)
    }

    /// Unit of the declared quantity.
    #[getter]
    fn unit(&self) -> Option<String> {
        self.0.unit().map(ToOwned::to_owned)
    }

    /// Free-form origin of the values.
    #[getter]
    fn provenance(&self) -> Option<String> {
        self.0.provenance().map(ToOwned::to_owned)
    }
}

/// Shared physical backing for one typed column.
#[pyclass(name = "AttributeValues", frozen, skip_from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyAttributeValues(pub(crate) molgfx::core::AttributeValues);

impl From<&molgfx::core::AttributeValues> for PyAttributeValues {
    fn from(value: &molgfx::core::AttributeValues) -> Self {
        Self(value.clone())
    }
}

#[pymethods]
impl PyAttributeValues {
    /// Physical kind of the backing.
    #[getter]
    fn kind(&self) -> PyAttributeKind {
        self.0.kind().into()
    }

    /// Rows in the column.
    #[getter]
    fn len(&self) -> usize {
        self.0.len()
    }

    #[getter]
    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Copies the scalars, absent when the backing holds another kind.
    fn copy_scalars_numpy<'py>(&self, py: Python<'py>) -> Option<Bound<'py, PyArray1<f32>>> {
        match &self.0 {
            molgfx::core::AttributeValues::Scalar(values) => Some(PyArray1::from_slice(py, values)),
            _ => None,
        }
    }

    /// Copies the category ids, absent when the backing holds another kind.
    fn copy_categories_numpy<'py>(&self, py: Python<'py>) -> Option<Bound<'py, PyArray1<u32>>> {
        match &self.0 {
            molgfx::core::AttributeValues::Category(values) => {
                Some(PyArray1::from_slice(py, values))
            }
            _ => None,
        }
    }

    /// Copies the packed vectors into one `(len, 3)` table, absent when the
    /// backing holds another kind.
    fn copy_vectors_numpy<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Option<Bound<'py, PyArray2<f32>>>> {
        let molgfx::core::AttributeValues::Vector(values) = &self.0 else {
            return Ok(None);
        };
        let rows = values.len();
        let table = PyArray2::zeros(py, (rows, 3), false);
        {
            let mut view = table.readwrite();
            let out = view
                .as_slice_mut()
                .map_err(|_| crate::error::value("vector table must stay contiguous"))?;
            for (slot, value) in out.chunks_exact_mut(3).zip(values.iter()) {
                slot.copy_from_slice(&[value[0], value[1], value[2]]);
            }
        }
        Ok(Some(table))
    }

    /// Copies the packed colors into one `(len, 4)` table, absent when the
    /// backing holds another kind.
    fn copy_colors_numpy<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Option<Bound<'py, PyArray2<u8>>>> {
        let molgfx::core::AttributeValues::Color(values) = &self.0 else {
            return Ok(None);
        };
        let rows = values.len();
        let table = PyArray2::zeros(py, (rows, 4), false);
        {
            let mut view = table.readwrite();
            let out = view
                .as_slice_mut()
                .map_err(|_| crate::error::value("color table must stay contiguous"))?;
            for (slot, value) in out.chunks_exact_mut(4).zip(values.iter()) {
                slot[0] = value.r;
                slot[1] = value.g;
                slot[2] = value.b;
                slot[3] = value.a;
            }
        }
        Ok(Some(table))
    }
}

/// One typed immutable column attached to an exact row domain.
#[pyclass(name = "AttributeColumn", frozen, skip_from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyAttributeColumn(pub(crate) molgfx::core::AttributeColumn);

impl From<&molgfx::core::AttributeColumn> for PyAttributeColumn {
    fn from(value: &molgfx::core::AttributeColumn) -> Self {
        Self(value.clone())
    }
}

#[pymethods]
impl PyAttributeColumn {
    /// Row table this column is aligned with.
    #[getter]
    fn domain(&self) -> PyRowDomain {
        PyRowDomain(self.0.domain())
    }

    /// Caller-facing column name.
    #[getter]
    fn name(&self) -> String {
        self.0.name().to_owned()
    }

    #[getter]
    fn descriptor(&self) -> PyAttributeDescriptor {
        PyAttributeDescriptor::from(self.0.descriptor())
    }

    /// Physical kind of the values.
    #[getter]
    fn kind(&self) -> PyAttributeKind {
        self.0.kind().into()
    }

    /// Native byte stride of one row.
    #[getter]
    fn stride(&self) -> u32 {
        self.0.stride()
    }

    /// Rows in the column.
    #[getter]
    fn len(&self) -> usize {
        self.0.len()
    }

    #[getter]
    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Content fingerprint of the column.
    #[getter]
    fn fingerprint(&self) -> u64 {
        self.0.fingerprint()
    }

    #[getter]
    fn values(&self) -> PyAttributeValues {
        PyAttributeValues::from(self.0.values())
    }
}

#[pymethods]
impl PyScene {
    #[pyo3(signature = (domain, style, order=0))]
    fn set_domain_visual(
        &mut self,
        domain: PyRowDomain,
        style: PyVisualStyle,
        order: i32,
    ) -> PyResult<()> {
        core(self.inner.set_domain_visual(
            domain.0,
            molgfx::core::VisualDescriptor::new(style.0).with_order(order),
        ))?;
        Ok(())
    }

    fn remove_domain_visual(&mut self, domain: PyRowDomain) -> bool {
        self.inner.remove_domain_visual(domain.0).is_some()
    }

    fn set_domain_visible(&mut self, domain: PyRowDomain, visible: bool) -> PyResult<bool> {
        core(self.inner.set_domain_visible(domain.0, visible))
    }

    fn domain_row_count(&self, domain: PyRowDomain) -> Option<u32> {
        self.inner.row_count(domain.0)
    }

    /// Scene-wide membership and content revision of the attribute table.
    #[getter]
    fn attribute_revision(&self) -> u64 {
        self.inner.attribute_revision()
    }

    /// Resolves a typed column by handle.
    fn attribute(&self, handle: PyAttributeHandle) -> Option<PyAttributeColumn> {
        self.inner.attribute(handle.0).map(Into::into)
    }

    /// Iterates typed columns in stable slot order.
    fn attributes(&self) -> Vec<(PyAttributeHandle, PyAttributeColumn)> {
        self.inner
            .attributes()
            .map(|(handle, column)| (handle.into(), column.into()))
            .collect()
    }

    /// Content revision and changed row range for one live column.
    fn attribute_change(&self, handle: PyAttributeHandle) -> Option<(u64, u32, u32)> {
        self.inner
            .attribute_change(handle.0)
            .map(|(revision, rows)| (revision, rows.start, rows.end))
    }

    /// Removes one column and invalidates its handle.
    fn remove_attribute(&mut self, handle: PyAttributeHandle) -> Option<PyAttributeColumn> {
        self.inner
            .remove_attribute(handle.0)
            .as_ref()
            .map(Into::into)
    }

    fn add_scalar_attribute_from_numpy(
        &mut self,
        domain: PyRowDomain,
        name: String,
        values: PyReadonlyArray1<'_, f32>,
    ) -> PyResult<PyAttributeHandle> {
        let values = values
            .as_slice()
            .map_err(|_| value("values must be C-contiguous float32"))?;
        self.add_native_attribute(
            domain,
            name,
            molgfx::core::AttributeValues::Scalar(Arc::from(values)),
        )
    }

    fn add_category_attribute_from_numpy(
        &mut self,
        domain: PyRowDomain,
        name: String,
        values: PyReadonlyArray1<'_, u32>,
    ) -> PyResult<PyAttributeHandle> {
        let values = values
            .as_slice()
            .map_err(|_| value("values must be C-contiguous uint32"))?;
        self.add_native_attribute(
            domain,
            name,
            molgfx::core::AttributeValues::Category(Arc::from(values)),
        )
    }

    fn add_vector_attribute_from_numpy(
        &mut self,
        domain: PyRowDomain,
        name: String,
        values: PyReadonlyArray2<'_, f32>,
    ) -> PyResult<PyAttributeHandle> {
        let values = vec3_rows("values", &values)?;
        self.add_native_attribute(
            domain,
            name,
            molgfx::core::AttributeValues::Vector(Arc::from(values)),
        )
    }

    fn add_color_attribute_from_numpy(
        &mut self,
        domain: PyRowDomain,
        name: String,
        values: PyReadonlyArray2<'_, u8>,
    ) -> PyResult<PyAttributeHandle> {
        validate_columns("values", values.shape(), 4)?;
        let values = values
            .as_slice()
            .map_err(|_| value("values must be C-contiguous uint8"))?
            .chunks_exact(4)
            .map(|row| molgfx::math::Rgba8::new(row[0], row[1], row[2], row[3]))
            .collect::<Vec<_>>();
        self.add_native_attribute(
            domain,
            name,
            molgfx::core::AttributeValues::Color(Arc::from(values)),
        )
    }
}

impl PyScene {
    fn add_native_attribute(
        &mut self,
        domain: PyRowDomain,
        name: String,
        values: molgfx::core::AttributeValues,
    ) -> PyResult<PyAttributeHandle> {
        core(molgfx::core::AttributeColumn::new(domain.0, name, values))
            .and_then(|column| core(self.inner.add_attribute(column)))
            .map(Into::into)
    }
}
