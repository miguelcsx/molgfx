//! Python class registration for generic compositions.

use super::PyGenericCompositionView;
use pyo3::prelude::*;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyGenericCompositionView>()
}
