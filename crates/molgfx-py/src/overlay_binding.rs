//! Immutable Python authoring values for overlay scene items.

use crate::binding::{error, selection};
use pyo3::exceptions::PyTypeError;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyModule};
mod segmentation_binding;
use segmentation_binding::PySegmentation;

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
#[pyclass(name = "Interaction", frozen, skip_from_py_object)]
struct PyInteraction(molgfx::InteractionSpec);

#[derive(Clone, Debug)]
#[pyclass(name = "Trajectory", frozen, skip_from_py_object)]
struct PyTrajectory(molgfx::TrajectorySpec);

#[pyclass(name = "Ellipsoid", frozen, skip_from_py_object)]
struct PyEllipsoid(molgfx::EllipsoidSpec);

/// One decoded coordinate frame, handed to `Scene.bind_trajectory`.
///
/// A frame is the value the renderer samples between, so it is its own type
/// rather than a loose triple of index, time and coordinates repeated per call.
#[derive(Clone, Debug)]
#[pyclass(name = "TrajectoryFrame", frozen, skip_from_py_object)]
pub(super) struct PyTrajectoryFrame(pub(super) molgfx::TrajectoryFrame);

#[pymethods]
impl PyTrajectoryFrame {
    #[new]
    #[pyo3(signature = (index, time, positions))]
    fn new(index: u64, time: f32, positions: Vec<[f32; 3]>) -> Self {
        Self(molgfx::TrajectoryFrame::new(
            index,
            time,
            std::sync::Arc::from(positions),
        ))
    }

    /// This frame's stable identity in its source.
    #[getter]
    fn index(&self) -> u64 {
        self.0.index()
    }

    /// Presentation time this frame was recorded at, in seconds.
    #[getter]
    fn time(&self) -> f32 {
        self.0.time_seconds()
    }

    /// The frame's coordinates, one per atom.
    #[getter]
    fn positions(&self) -> Vec<[f32; 3]> {
        self.0.positions().to_vec()
    }
}

#[pyfunction(name = "frame")]
#[pyo3(signature = (index, time, positions))]
fn trajectory_frame(index: u64, time: f32, positions: Vec<[f32; 3]>) -> PyTrajectoryFrame {
    PyTrajectoryFrame::new(index, time, positions)
}

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
        _ => Err(PyTypeError::new_err("unknown overlay interaction kind")),
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
#[pyo3(signature = (*, source, dimensions, voxel_to_world=None))]
fn volume(
    source: &PyDataSource,
    dimensions: (u32, u32, u32),
    voxel_to_world: Option<[f32; 16]>,
) -> PyVolume {
    let mut affine = [
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ];
    if let Some(matrix) = voxel_to_world {
        affine = matrix;
    }
    PyVolume(molgfx::VolumeSpec {
        source: source.0.clone(),
        dimensions: [dimensions.0, dimensions.1, dimensions.2],
        voxel_to_world: affine,
        presentations: Vec::new(),
        region: None,
    })
}

#[pymethods]
impl PyVolume {
    #[pyo3(signature = (isovalue, *, color=(49,104,142), opacity=1.0, style="solid", width=0.1))]
    fn isosurface(
        &self,
        isovalue: f32,
        color: (u8, u8, u8),
        opacity: f32,
        style: &str,
        width: f32,
    ) -> PyResult<Self> {
        let style = match style {
            "solid" => molgfx::IsoStyle::Solid,
            "mesh" => molgfx::IsoStyle::Mesh {
                line_width_voxels: width,
            },
            "dots" => molgfx::IsoStyle::Dots {
                dot_radius_voxels: width,
            },
            _ => {
                return Err(PyTypeError::new_err(
                    "isosurface style must be solid, mesh, or dots",
                ));
            }
        };
        let mut value = self.clone();
        value
            .0
            .presentations
            .push(molgfx::VolumePresentation::Isosurface {
                isovalue,
                color: rgb(color),
                opacity,
                style,
            });
        Ok(value)
    }

    #[pyo3(signature = (transfer, *, opacity_scale=2.0, step_scale=0.65, medium=false))]
    fn direct(
        &self,
        transfer: Vec<(f32, (u8, u8, u8), f32)>,
        opacity_scale: f32,
        step_scale: f32,
        medium: bool,
    ) -> Self {
        let transfer = transfer
            .into_iter()
            .map(|(value, color, opacity)| molgfx::VolumeTransferPoint {
                value,
                color: rgb(color),
                opacity,
            })
            .collect();
        let presentation = if medium {
            molgfx::VolumePresentation::Medium {
                transfer,
                opacity_scale,
                step_scale,
            }
        } else {
            molgfx::VolumePresentation::Direct {
                transfer,
                opacity_scale,
                step_scale,
            }
        };
        let mut value = self.clone();
        value.0.presentations.push(presentation);
        value
    }

    #[pyo3(signature = (*, point, normal, ramp="viridis", domain=(0.0,1.0)))]
    fn slice(&self, point: [f32; 3], normal: [f32; 3], ramp: &str, domain: (f32, f32)) -> Self {
        let mut value = self.clone();
        value
            .0
            .presentations
            .push(molgfx::VolumePresentation::Slice {
                point,
                normal,
                ramp: ramp.into(),
                domain: [domain.0, domain.1],
            });
        value
    }

    #[pyo3(signature = (isovalue, *, color=(49,104,142), opacity=1.0))]
    fn liquid_surface(&self, isovalue: f32, color: (u8, u8, u8), opacity: f32) -> Self {
        let mut value = self.clone();
        value
            .0
            .presentations
            .push(molgfx::VolumePresentation::LiquidSurface {
                isovalue,
                color: rgb(color),
                opacity,
            });
        value
    }

    fn region(&self, minimum: [u32; 3], maximum: [u32; 3]) -> Self {
        let mut value = self.clone();
        value.0.region = Some(molgfx::VolumeRegion { minimum, maximum });
        value
    }
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
fn explicit(kind: &str, first: &PyAnchor, second: &PyAnchor) -> PyResult<PyInteraction> {
    Ok(PyInteraction(molgfx::interaction::explicit(
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

#[pyfunction]
#[pyo3(signature = (*, structure, target, scale=None, color=None, opacity=None))]
fn adp(
    structure: &Bound<'_, PyAny>,
    target: &Bound<'_, PyAny>,
    scale: Option<f32>,
    color: Option<(u8, u8, u8)>,
    opacity: Option<f32>,
) -> PyResult<PyEllipsoid> {
    let structure = molgfx::StructureId::new(crate::id_binding::structure_id(structure)?);
    let mut spec = molgfx::ellipsoid::adp(structure, selection(target)?.into());
    if let Some(scale) = scale {
        spec.scale = scale;
    }
    if let Some(color) = color {
        spec.color = rgb(color);
    }
    if let Some(opacity) = opacity {
        spec.opacity = opacity;
    }
    Ok(PyEllipsoid(spec))
}

pub(super) fn add_item(
    item: &Bound<'_, PyAny>,
    scene: &mut molgfx::Scene,
) -> PyResult<(crate::id_binding::PySceneId, molgfx::schema::PatchOperation)> {
    if let Ok(item) = item.extract::<PyRef<'_, PySegmentation>>() {
        let id = scene.add_segmentation(item.0.clone()).map_err(error)?;
        return Ok((
            crate::id_binding::PySceneId::Segmentation(id),
            molgfx::schema::PatchOperation::AddSegmentation {
                id,
                segmentation: item.0.clone(),
            },
        ));
    }
    if let Ok(item) = item.extract::<PyRef<'_, PyVolume>>() {
        let mut value = molgfx::density::volume(item.0.source.clone(), item.0.dimensions)
            .affine(item.0.voxel_to_world);
        for presentation in &item.0.presentations {
            value = value.presentation(presentation.clone());
        }
        if let Some(region) = item.0.region {
            value = value.region(region.minimum, region.maximum);
        }
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
    if let Ok(item) = item.extract::<PyRef<'_, PyInteraction>>() {
        let id = scene.add(item.0.clone()).map_err(error)?;
        return Ok((
            crate::id_binding::PySceneId::Interaction(id.get()),
            molgfx::schema::PatchOperation::AddInteraction {
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
    if let Ok(item) = item.extract::<PyRef<'_, PyEllipsoid>>() {
        let id = scene.add(item.0.clone()).map_err(error)?;
        return Ok((
            crate::id_binding::PySceneId::Ellipsoids(id.get()),
            molgfx::schema::PatchOperation::AddEllipsoids {
                id,
                spec: item.0.clone(),
            },
        ));
    }
    Err(PyTypeError::new_err(
        "expected a representation or overlay scene item",
    ))
}

fn namespace<'py>(module: &Bound<'py, PyModule>, name: &str) -> PyResult<Bound<'py, PyModule>> {
    PyModule::new(module.py(), name)
}

pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    segmentation_binding::register(module)?;
    module.add_class::<PyDataSource>()?;
    module.add_class::<PyAnchor>()?;
    module.add_class::<PyVolume>()?;
    module.add_class::<PyLabel>()?;
    module.add_class::<PyMeasurement>()?;
    module.add_class::<PyInteraction>()?;
    module.add_class::<PyTrajectory>()?;
    module.add_class::<PyTrajectoryFrame>()?;
    module.add_class::<PyEllipsoid>()?;
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
    let ellipsoid = namespace(module, "ellipsoid")?;
    ellipsoid.add_function(wrap_pyfunction!(adp, &ellipsoid)?)?;
    module.add_submodule(&ellipsoid)?;
    let trajectory = namespace(module, "trajectory")?;
    trajectory.add_function(wrap_pyfunction!(bind_trajectory, &trajectory)?)?;
    trajectory.add_function(wrap_pyfunction!(trajectory_frame, &trajectory)?)?;
    trajectory.add_class::<PyTrajectoryFrame>()?;
    module.add_submodule(&trajectory)
}
