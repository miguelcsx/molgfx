//! Ownership-explicit transfer of Rust image storage to `NumPy`.

use crate::error::value;
use numpy::ndarray::Array3;
use numpy::{IntoPyArray, PyArray3, PyArrayMethods};
use pyo3::prelude::*;

pub(super) fn transfer_image_array(
    py: Python<'_>,
    image: molgfx::Image,
) -> PyResult<Bound<'_, PyArray3<u8>>> {
    let height =
        usize::try_from(image.height).map_err(|_| value("image height exceeds Python limits"))?;
    let width =
        usize::try_from(image.width).map_err(|_| value("image width exceeds Python limits"))?;
    let array = Array3::from_shape_vec((height, width, 4), image.pixels)
        .map_err(|error| value(error.to_string()))?;
    let result = array.into_pyarray(py);
    let _readonly = result.readwrite().make_nonwriteable();
    Ok(result)
}
