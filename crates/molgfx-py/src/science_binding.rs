//! Immutable Python authoring values for scientific scene items.

use crate::binding::{error, selection};
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyModule};

#[derive(Clone, Debug)]
#[pyclass(name = "DataSource", frozen, skip_from_py_object)]
struct PyDataSource(molgfx::schema::DataSource);

#[derive(Clone, Debug)]
#[pyclass(name = "Anchor", frozen, skip_from_py_object)]
pub(super) struct PyAnchor(molgfx::Anchor);

#[derive(Clone, Debug)]
#[pyclass(name = "Volume", frozen, skip_from_py_object)]
struct PyVolume(molgfx::VolumeSpec);

#[derive(Clone, Debug)]
#[pyclass(name = "Label", frozen, skip_from_py_object)]
struct PyLabel(molgfx::AnnotationSpec);

#[derive(Clone, Debug)]
#[pyclass(name = "Measurement", frozen, skip_from_py_object)]
struct PyMeasurement(molgfx::MeasurementSpec);

#[derive(Clone, Debug)]
#[pyclass(name = "ScientificInteraction", frozen, skip_from_py_object)]
struct PyScientificInteraction(molgfx::ScientificInteractionSpec);

#[derive(Clone, Debug)]
#[pyclass(name = "Trajectory", frozen, skip_from_py_object)]
struct PyTrajectory(molgfx::TrajectorySpec);

fn rgb(value: (u8, u8, u8)) -> molgfx::Color {
    molgfx::Color::rgb(value.0, value.1, value.2)
}

fn interaction_kind(value: &str) -> PyResult<molgfx::InteractionKind> {
    match value {
        "hydrogen_bond" => Ok(molgfx::InteractionKind::HydrogenBond),
        "salt_bridge" => Ok(molgfx::InteractionKind::SaltBridge),
        "pi_stacking" => Ok(molgfx::InteractionKind::PiStacking),
        "cation_pi" => Ok(molgfx::InteractionKind::CationPi),
        "hydrophobic" => Ok(molgfx::InteractionKind::Hydrophobic),
        "metal_coordination" => Ok(molgfx::InteractionKind::MetalCoordination),
        "contact" => Ok(molgfx::InteractionKind::Contact),
        _ => Err(PyTypeError::new_err("unknown scientific interaction kind")),
    }
}

#[pyfunction]
#[pyo3(signature = (content_hash, *, uri=None, format=None))]
fn source(content_hash: &str, uri: Option<&str>, format: Option<&str>) -> PyDataSource {
    let mut value = molgfx::schema::DataSource::new(content_hash);
    if let Some(uri) = uri {
        value = value.uri(uri);
    }
    if let Some(format) = format {
        value = value.format(format);
    }
    PyDataSource(value)
}

#[pyfunction]
fn world(position: (f32, f32, f32)) -> PyAnchor {
    PyAnchor(molgfx::Anchor::World {
        position: [position.0, position.1, position.2],
    })
}

#[pyfunction(name = "selection")]
#[pyo3(signature = (*, structure, target))]
fn selection_anchor(structure: &Bound<'_, PyAny>, target: &Bound<'_, PyAny>) -> PyResult<PyAnchor> {
    Ok(PyAnchor(molgfx::Anchor::Selection {
        structure: molgfx::StructureId::new(crate::id_binding::structure_id(structure)?),
        selection: selection(target)?.into(),
    }))
}

#[pyfunction]
#[pyo3(signature = (*, source, dimensions, spacing=(1.0, 1.0, 1.0), origin=(0.0, 0.0, 0.0), isovalue=1.0, color=(49, 104, 142)))]
fn volume(
    source: &PyDataSource,
    dimensions: (u32, u32, u32),
    spacing: (f32, f32, f32),
    origin: (f32, f32, f32),
    isovalue: f32,
    color: (u8, u8, u8),
) -> PyVolume {
    PyVolume(molgfx::VolumeSpec {
        source: source.0.clone(),
        dimensions: [dimensions.0, dimensions.1, dimensions.2],
        spacing: [spacing.0, spacing.1, spacing.2],
        origin: [origin.0, origin.1, origin.2],
        isovalue,
        color: rgb(color),
    })
}

#[pyfunction]
#[pyo3(signature = (*, anchor, text, color=(255, 255, 255)))]
fn label(anchor: &PyAnchor, text: &str, color: (u8, u8, u8)) -> PyLabel {
    PyLabel(molgfx::AnnotationSpec {
        anchor: anchor.0.clone(),
        text: text.into(),
        color: rgb(color),
    })
}

#[pyfunction]
fn distance(first: &PyAnchor, second: &PyAnchor) -> PyMeasurement {
    PyMeasurement(molgfx::measurement::distance(
        first.0.clone(),
        second.0.clone(),
    ))
}

#[pyfunction]
fn angle(first: &PyAnchor, middle: &PyAnchor, last: &PyAnchor) -> PyMeasurement {
    PyMeasurement(molgfx::measurement::angle(
        first.0.clone(),
        middle.0.clone(),
        last.0.clone(),
    ))
}

#[pyfunction]
fn dihedral(
    first: &PyAnchor,
    second: &PyAnchor,
    third: &PyAnchor,
    fourth: &PyAnchor,
) -> PyMeasurement {
    PyMeasurement(molgfx::measurement::dihedral(
        first.0.clone(),
        second.0.clone(),
        third.0.clone(),
        fourth.0.clone(),
    ))
}

#[pyfunction]
#[pyo3(signature = (*, kind, first, second))]
fn explicit(kind: &str, first: &PyAnchor, second: &PyAnchor) -> PyResult<PyScientificInteraction> {
    Ok(PyScientificInteraction(molgfx::interaction::explicit(
        interaction_kind(kind)?,
        first.0.clone(),
        second.0.clone(),
    )))
}

#[pyfunction(name = "bind")]
#[pyo3(signature = (*, structure, source, frame_count, time_step=None, time_unit=None))]
fn bind_trajectory(
    structure: &Bound<'_, PyAny>,
    source: &PyDataSource,
    frame_count: u64,
    time_step: Option<f64>,
    time_unit: Option<&str>,
) -> PyResult<PyTrajectory> {
    Ok(PyTrajectory(molgfx::TrajectorySpec {
        structure: molgfx::StructureId::new(crate::id_binding::structure_id(structure)?),
        source: source.0.clone(),
        frame_count,
        time_step,
        time_unit: time_unit.map(Into::into),
    }))
}

pub(super) fn add_item(
    item: &Bound<'_, PyAny>,
    scene: &mut molgfx::Scene,
) -> PyResult<(crate::id_binding::PySceneId, molgfx::schema::PatchOperation)> {
    if let Ok(item) = item.extract::<PyRef<'_, PyVolume>>() {
        let value = molgfx::density::volume(item.0.source.clone(), item.0.dimensions)
            .spacing(item.0.spacing)
            .origin(item.0.origin)
            .isovalue(item.0.isovalue)
            .color(item.0.color);
        let id = scene.add(value).map_err(error)?;
        return Ok((
            crate::id_binding::PySceneId::Volume(id.get()),
            molgfx::schema::PatchOperation::AddVolume {
                id,
                volume: item.0.clone(),
            },
        ));
    }
    if let Ok(item) = item.extract::<PyRef<'_, PyLabel>>() {
        let value = molgfx::annotation::label(item.0.anchor.clone(), item.0.text.clone())
            .color(item.0.color);
        let id = scene.add(value).map_err(error)?;
        return Ok((
            crate::id_binding::PySceneId::Annotation(id.get()),
            molgfx::schema::PatchOperation::AddAnnotation {
                id,
                annotation: item.0.clone(),
            },
        ));
    }
    if let Ok(item) = item.extract::<PyRef<'_, PyMeasurement>>() {
        let id = scene.add(item.0.clone()).map_err(error)?;
        return Ok((
            crate::id_binding::PySceneId::Measurement(id.get()),
            molgfx::schema::PatchOperation::AddMeasurement {
                id,
                measurement: item.0.clone(),
            },
        ));
    }
    if let Ok(item) = item.extract::<PyRef<'_, PyScientificInteraction>>() {
        let id = scene.add(item.0.clone()).map_err(error)?;
        return Ok((
            crate::id_binding::PySceneId::ScientificInteraction(id.get()),
            molgfx::schema::PatchOperation::AddScientificInteraction {
                id,
                interaction: item.0.clone(),
            },
        ));
    }
    if let Ok(item) = item.extract::<PyRef<'_, PyTrajectory>>() {
        let id = scene.add(item.0.clone()).map_err(error)?;
        return Ok((
            crate::id_binding::PySceneId::Trajectory(id.get()),
            molgfx::schema::PatchOperation::AddTrajectory {
                id,
                trajectory: item.0.clone(),
            },
        ));
    }
    Err(PyTypeError::new_err(
        "expected a representation or scientific scene item",
    ))
}

fn namespace<'py>(module: &Bound<'py, PyModule>, name: &str) -> PyResult<Bound<'py, PyModule>> {
    PyModule::new(module.py(), name)
}

pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyDataSource>()?;
    module.add_class::<PyAnchor>()?;
    module.add_class::<PyVolume>()?;
    module.add_class::<PyLabel>()?;
    module.add_class::<PyMeasurement>()?;
    module.add_class::<PyScientificInteraction>()?;
    module.add_class::<PyTrajectory>()?;
    let data = namespace(module, "data")?;
    data.add_function(wrap_pyfunction!(source, &data)?)?;
    module.add_submodule(&data)?;
    let annotation = namespace(module, "annotation")?;
    annotation.add_function(wrap_pyfunction!(world, &annotation)?)?;
    annotation.add_function(wrap_pyfunction!(selection_anchor, &annotation)?)?;
    annotation.add_function(wrap_pyfunction!(label, &annotation)?)?;
    module.add_submodule(&annotation)?;
    let density = namespace(module, "density")?;
    density.add_function(wrap_pyfunction!(volume, &density)?)?;
    module.add_submodule(&density)?;
    let measurement = namespace(module, "measurement")?;
    measurement.add_function(wrap_pyfunction!(distance, &measurement)?)?;
    measurement.add_function(wrap_pyfunction!(angle, &measurement)?)?;
    measurement.add_function(wrap_pyfunction!(dihedral, &measurement)?)?;
    module.add_submodule(&measurement)?;
    let interaction = namespace(module, "interaction")?;
    interaction.add_function(wrap_pyfunction!(explicit, &interaction)?)?;
    module.add_submodule(&interaction)?;
    let trajectory = namespace(module, "trajectory")?;
    trajectory.add_function(wrap_pyfunction!(bind_trajectory, &trajectory)?)?;
    module.add_submodule(&trajectory)
}
