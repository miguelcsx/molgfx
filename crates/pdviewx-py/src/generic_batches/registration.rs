//! Python class registration for generic scene tables.

use super::{PyAnalyticTemplate, PyRelationPattern, PyRowDomain};
use pyo3::prelude::*;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyRowDomain>()?;
    module.add_class::<PyRelationPattern>()?;
    module.add_class::<PyAnalyticTemplate>()
}
