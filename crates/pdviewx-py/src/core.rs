//! Registration hub for the declarative core binding modules.

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

pub(crate) use handles::*;
pub(crate) use representation::*;
pub(crate) use scene::PyScene;
pub(crate) use selection::*;
pub(crate) use volumes::*;

use pyo3::prelude::*;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    handles::register(module)?;
    selection::register(module)?;
    representation::register(module)?;
    volumes::register(module)?;
    provenance::register(module)?;
    scene::register(module)?;
    module.add("Selection", module.py().get_type::<PySelectionHandle>())?;
    module.add_function(wrap_pyfunction!(selection::select, module)?)
}
