//! Registration hub for the declarative core binding modules.

#[path = "atom_property.rs"]
mod atom_property;
#[path = "generic_batches/mod.rs"]
mod generic_batches;
#[path = "handles.rs"]
mod handles;
#[path = "provenance.rs"]
mod provenance;
#[path = "representation.rs"]
mod representation;
#[path = "scene.rs"]
mod scene;
#[path = "selection.rs"]
mod selection;
#[path = "volumes.rs"]
mod volumes;

pub(crate) use generic_batches::{PyAnalyticTemplate, PyRelationPattern, PyRowDomain};
pub(crate) use handles::*;
pub(crate) use representation::*;
pub(crate) use scene::PyScene;
pub(crate) use selection::*;
pub(crate) use volumes::*;

use pyo3::prelude::*;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    handles::register(module)?;
    atom_property::register(module)?;
    generic_batches::register(module)?;
    selection::register(module)?;
    representation::register(module)?;
    volumes::register(module)?;
    provenance::register(module)?;
    scene::register(module)?;
    module.add("Selection", module.py().get_type::<PySelectionHandle>())?;
    module.add_function(wrap_pyfunction!(selection::select, module)?)
}
