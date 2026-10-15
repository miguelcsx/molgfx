//! Interaction authoring values keep chemical analysis in the caller.

use super::{PyAnchor, PyDataSource};
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyModule};
#[derive(Clone, Debug)]
#[pyclass(name = "Interaction", frozen, skip_from_py_object)]
pub(super) struct PyInteraction(pub(super) molgfx::InteractionSpec);

fn interaction_kind(value: &str) -> PyResult<molgfx::InteractionKind> {
    match value {
        "hydrogen_bond" => Ok(molgfx::InteractionKind::HydrogenBond),
        "salt_bridge" => Ok(molgfx::InteractionKind::SaltBridge),
        "pi_stacking" => Ok(molgfx::InteractionKind::PiStacking),
        "cation_pi" => Ok(molgfx::InteractionKind::CationPi),
        "hydrophobic" => Ok(molgfx::InteractionKind::Hydrophobic),
        "metal_coordination" => Ok(molgfx::InteractionKind::MetalCoordination),
        "contact" => Ok(molgfx::InteractionKind::Contact),
        _ => Err(PyTypeError::new_err("unknown overlay interaction kind")),
    }
}

#[pyfunction]
#[pyo3(signature = (*, kind, first, second))]
fn explicit(kind: &str, first: &PyAnchor, second: &PyAnchor) -> PyResult<PyInteraction> {
    Ok(PyInteraction(molgfx::interaction::explicit(
        interaction_kind(kind)?,
        first.0.clone(),
        second.0.clone(),
    )))
}

#[pyfunction]
#[pyo3(signature = (*, structure, source, target="all", width=1.5, color=(128, 204, 255), opacity=1.0, geometry=None))]
fn hydrogen_bonds(
    structure: &Bound<'_, PyAny>,
    source: &PyDataSource,
    target: &str,
    width: f32,
    color: (u8, u8, u8),
    opacity: f32,
    geometry: Option<(f32, f32, Option<f32>)>,
) -> PyResult<PyInteraction> {
    let mut value = molgfx::HydrogenBonds::new(
        source.0.clone(),
        molgfx::StructureId::new(crate::id_binding::structure_id(structure)?),
    )
    .selection(target);
    value.width_pixels = width;
    value.color = super::rgb(color);
    value.opacity = opacity;
    value.geometry =
        geometry.map(
            |(distance, angle, hydrogen_distance)| molgfx::HydrogenBondGeometry {
                maximum_endpoint_distance: distance,
                minimum_angle_degrees: angle,
                maximum_pivot_end_distance: hydrogen_distance,
            },
        );
    Ok(PyInteraction(value.interaction()))
}

pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(explicit, module)?)?;
    module.add_function(wrap_pyfunction!(hydrogen_bonds, module)?)?;
    Ok(())
}
