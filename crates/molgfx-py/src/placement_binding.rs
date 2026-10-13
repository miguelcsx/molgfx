//! `molgfx.place`: row-major rigid transforms placed through the facade.

use crate::scene_binding::PyScene;
use pyo3::prelude::*;
use pyo3::types::{PyModule, PyTuple};

type Rotation = [[f32; 3]; 3];

/// The 16 column-major floats for `x' = rotation @ x + translation`.
#[pyfunction]
fn from_rotation_translation(rotation: Rotation, translation: [f32; 3]) -> Vec<f32> {
    molgfx::placement::rigid(rotation, translation).to_vec()
}

fn rigid_parts(superposition: &Bound<'_, PyAny>) -> PyResult<(Rotation, [f32; 3])> {
    if let Ok(pair) = superposition.cast::<PyTuple>() {
        return pair.extract();
    }
    Ok((
        superposition.getattr("rotation")?.extract()?,
        superposition.getattr("translation")?.extract()?,
    ))
}

/// Column-major placement for a molframe `Superposition` (`R @ x + t`).
#[pyfunction]
fn from_superposition(superposition: &Bound<'_, PyAny>) -> PyResult<Vec<f32>> {
    let (rotation, translation) = rigid_parts(superposition)?;
    Ok(molgfx::placement::rigid(rotation, translation).to_vec())
}

/// Place a structure with a molframe `Superposition` or a `(R, t)` pair.
#[pyfunction]
#[pyo3(signature = (scene, superposition, *, structure=None))]
fn apply(
    scene: &Bound<'_, PyScene>,
    superposition: &Bound<'_, PyAny>,
    structure: Option<&Bound<'_, PyAny>>,
) -> PyResult<Py<PyAny>> {
    let (rotation, translation) = rigid_parts(superposition)?;
    PyScene::place(
        scene,
        scene.py(),
        None,
        Some(rotation),
        Some(translation),
        structure,
    )
}

pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    let place = PyModule::new(module.py(), "place")?;
    place.add_function(wrap_pyfunction!(from_rotation_translation, &place)?)?;
    place.add_function(wrap_pyfunction!(from_superposition, &place)?)?;
    place.add_function(wrap_pyfunction!(apply, &place)?)?;
    module.add_submodule(&place)
}
