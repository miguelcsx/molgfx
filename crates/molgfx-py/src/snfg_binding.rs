//! SNFG symbol lookup: the glyph and colour a CCD component draws with.

use molgfx_geometry::snfg_component_symbol;
use pyo3::prelude::*;
use pyo3::types::PyModule;

/// `(shape, "#rrggbb", abbreviation)` for a CCD component id, or `None`.
#[pyfunction]
fn glyph(component: &str) -> Option<(&'static str, String, &'static str)> {
    snfg_component_symbol(&component.to_ascii_uppercase()).map(|(shape, color, abbreviation)| {
        (shape.stable_name(), format!("#{color:06x}"), abbreviation)
    })
}

pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    let snfg = PyModule::new(module.py(), "snfg")?;
    snfg.add_function(wrap_pyfunction!(glyph, &snfg)?)?;
    module.add_submodule(&snfg)
}
