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
use numpy::{PyReadonlyArray1, PyReadonlyArray2, PyUntypedArrayMethods};
use pyo3::prelude::*;
use std::sync::Arc;

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
            pdviewx::VisualDescriptor::new(style.0).with_order(order),
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
            pdviewx::AttributeValues::Scalar(Arc::from(values)),
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
            pdviewx::AttributeValues::Category(Arc::from(values)),
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
            pdviewx::AttributeValues::Vector(Arc::from(values)),
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
            .map(|row| pdviewx::Rgba8::new(row[0], row[1], row[2], row[3]))
            .collect::<Vec<_>>();
        self.add_native_attribute(
            domain,
            name,
            pdviewx::AttributeValues::Color(Arc::from(values)),
        )
    }
}

impl PyScene {
    fn add_native_attribute(
        &mut self,
        domain: PyRowDomain,
        name: String,
        values: pdviewx::AttributeValues,
    ) -> PyResult<PyAttributeHandle> {
        core(pdviewx::AttributeColumn::new(domain.0, name, values))
            .and_then(|column| core(self.inner.add_attribute(column)))
            .map(Into::into)
    }
}
