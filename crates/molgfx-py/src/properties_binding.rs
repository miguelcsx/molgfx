//! `molgfx.properties`: scalar columns whose identity is derived from the data.

use crate::authoring_binding::{PyColorSpec, PyScalarProperty};
use crate::binding::error;
use crate::scene_binding::PyScene;
use numpy::PyReadonlyArray1;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyModule;
use std::sync::Arc;

/// One float32 column from an array or sequence, copied once.
fn column(values: &Bound<'_, PyAny>) -> PyResult<Arc<[f32]>> {
    if let Ok(array) = values.extract::<PyReadonlyArray1<'_, f32>>() {
        let view = array.as_array();
        return Ok(match view.as_slice() {
            Some(flat) => Arc::from(flat),
            None => view.iter().copied().collect(),
        });
    }
    Ok(Arc::from(values.extract::<Vec<f32>>()?))
}

fn derive(
    scene: &Bound<'_, PyScene>,
    values: &Bound<'_, PyAny>,
    name: Option<&str>,
    domain: Option<(f32, f32)>,
    structure: Option<&Bound<'_, PyAny>>,
) -> PyResult<(molgfx::ScalarProperty, [f32; 2])> {
    let values = column(values)?;
    let identity = match structure {
        Some(structure) => crate::id_binding::structure_id(structure)?,
        None => scene.borrow().structure_id,
    };
    let mut binding =
        molgfx::ScalarPropertyBinding::derived(molgfx::StructureId::new(identity), name, values)
            .map_err(|failure| PyValueError::new_err(failure.to_string()))?;
    if let Some((low, high)) = domain {
        binding = binding.domain([low, high]);
    }
    let span = binding.display_domain();
    let property = scene
        .borrow_mut()
        .inner
        .bind_property(binding)
        .map_err(error)?;
    Ok((property, span))
}

/// Bind one value per atom with a source hash derived from the data.
#[pyfunction]
#[pyo3(signature = (scene, values, *, name=None, domain=None, structure=None))]
fn bind(
    scene: &Bound<'_, PyScene>,
    values: &Bound<'_, PyAny>,
    name: Option<&str>,
    domain: Option<(f32, f32)>,
    structure: Option<&Bound<'_, PyAny>>,
) -> PyResult<PyScalarProperty> {
    derive(scene, values, name, domain, structure).map(|(property, _)| PyScalarProperty(property))
}

/// A colour mapping one per-atom value through a ramp over the data's domain.
#[pyfunction]
#[pyo3(signature = (scene, values, *, ramp="viridis", domain=None, name=None))]
fn by_array(
    scene: &Bound<'_, PyScene>,
    values: &Bound<'_, PyAny>,
    ramp: &str,
    domain: Option<(f32, f32)>,
    name: Option<&str>,
) -> PyResult<PyColorSpec> {
    let (property, span) = derive(scene, values, name, domain, None)?;
    Ok(PyColorSpec(molgfx::color::property(
        property,
        ramp,
        span,
        None,
        molgfx::Color::rgb(128, 128, 128),
    )))
}

pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    let properties = PyModule::new(module.py(), "properties")?;
    properties.add_function(wrap_pyfunction!(bind, &properties)?)?;
    properties.add_function(wrap_pyfunction!(by_array, &properties)?)?;
    module.add_submodule(&properties)
}
