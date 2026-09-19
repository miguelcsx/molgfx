//! Shared `NumPy` shape and contiguity validation.

use crate::error::value;
use numpy::{PyReadonlyArray2, PyUntypedArrayMethods};
use pyo3::prelude::*;

pub(super) fn ordered_rows(namespace: u64, count: usize) -> PyResult<molgfx::core::SourceRows> {
    let count = u32::try_from(count).map_err(|_| value("row count exceeds u32"))?;
    Ok(molgfx::core::SourceRows::ordered(
        molgfx::core::SourceNamespace(namespace),
        count,
    ))
}

pub(super) fn validate_columns(name: &str, shape: &[usize], columns: usize) -> PyResult<()> {
    if shape.len() == 2 && shape[1] == columns {
        Ok(())
    } else {
        Err(value(format!("{name} must have shape (N, {columns})")))
    }
}

pub(super) fn contiguous<'a>(
    name: &str,
    values: &'a PyReadonlyArray2<'a, f32>,
) -> PyResult<&'a [f32]> {
    values
        .as_slice()
        .map_err(|_| value(format!("{name} must be C-contiguous float32")))
}

pub(super) fn vec3_rows(name: &str, values: &PyReadonlyArray2<'_, f32>) -> PyResult<Vec<[f32; 3]>> {
    validate_columns(name, values.shape(), 3)?;
    Ok(contiguous(name, values)?
        .chunks_exact(3)
        .map(|row| [row[0], row[1], row[2]])
        .collect())
}
