//! Immutable Python authoring values for overlay scene items.

use crate::binding::{error, rgb, selection};
use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyModule};
mod segmentation_binding;
use segmentation_binding::PySegmentation;
mod volume_binding;
use volume_binding::{PyVolume, volume};
mod interaction_binding;
use interaction_binding::PyInteraction;

#[derive(Clone, Debug)]
#[pyclass(name = "DataSource", frozen, skip_from_py_object)]
pub(super) struct PyDataSource(pub(super) molgfx::schema::DataSource);

#[derive(Clone, Debug)]
#[pyclass(name = "Anchor", frozen, skip_from_py_object)]
pub(super) struct PyAnchor(molgfx::Anchor);

#[derive(Clone, Debug)]
#[pyclass(name = "Label", frozen, skip_from_py_object)]
struct PyLabel(molgfx::AnnotationSpec);

#[derive(Clone, Debug)]
#[pyclass(name = "Measurement", frozen, skip_from_py_object)]
struct PyMeasurement(molgfx::MeasurementSpec);

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
    pub(super) fn new(index: u64, time: f64, positions: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self(molgfx::TrajectoryFrame::new(
            index,
            time,
            frame_positions(positions)?,
        )))
    }

    /// This frame's stable identity in its source.
    #[getter]
    fn index(&self) -> u64 {
        self.0.index()
    }

    /// Presentation time this frame was recorded at, in seconds.
    #[getter]
    fn time(&self) -> f64 {
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
fn trajectory_frame(
    index: u64,
    time: f64,
    positions: &Bound<'_, PyAny>,
) -> PyResult<PyTrajectoryFrame> {
    PyTrajectoryFrame::new(index, time, positions)
}

/// One frame's coordinates from an `(atoms, 3)` array or a nested sequence.
///
/// A float32 `NumPy` array (or anything exposing one) is read straight from its
/// buffer in a single copy; float64 arrays are narrowed the same way. Only a
/// plain list of lists takes the per-element path.
fn frame_positions(positions: &Bound<'_, PyAny>) -> PyResult<std::sync::Arc<[[f32; 3]]>> {
    use numpy::{PyReadonlyArray2, PyUntypedArrayMethods as _};
    if let Ok(array) = positions.extract::<PyReadonlyArray2<'_, f32>>() {
        if array.shape()[1] != 3 {
            return Err(PyValueError::new_err(
                "frame positions must have shape (atoms, 3)",
            ));
        }
        let view = array.as_array();
        return Ok(match view.as_slice() {
            Some(flat) => flat.as_chunks::<3>().0.iter().copied().collect(),
            None => view
                .rows()
                .into_iter()
                .map(|row| [row[0], row[1], row[2]])
                .collect(),
        });
    }
    if positions.extract::<PyReadonlyArray2<'_, f64>>().is_ok() {
        // The renderer stores single precision: NumPy narrows, and the
        // float32 path above reads the result.
        return frame_positions(&positions.call_method1("astype", ("float32",))?);
    }
    let rows: Vec<[f32; 3]> = positions.extract()?;
    Ok(std::sync::Arc::from(rows))
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

fn anchor_repr(anchor: &molgfx::Anchor) -> String {
    match anchor {
        molgfx::Anchor::World { position } => format!(
            "annotation.world(({}, {}, {}))",
            position[0], position[1], position[2]
        ),
        molgfx::Anchor::Selection {
            structure,
            selection,
        } => format!(
            "annotation.selection(structure={}, target={:?})",
            structure.get(),
            selection.source()
        ),
    }
}

#[pymethods]
impl PyAnchor {
    /// `"world"` for a fixed point or `"selection"` for a query centroid.
    #[getter]
    fn kind(&self) -> &'static str {
        match self.0 {
            molgfx::Anchor::World { .. } => "world",
            molgfx::Anchor::Selection { .. } => "selection",
        }
    }

    /// The world position of a `world` anchor, else `None`.
    #[getter]
    fn position(&self) -> Option<(f32, f32, f32)> {
        match &self.0 {
            molgfx::Anchor::World { position } => Some((position[0], position[1], position[2])),
            molgfx::Anchor::Selection { .. } => None,
        }
    }

    /// The structure id of a `selection` anchor, else `None`.
    #[getter]
    fn structure(&self) -> Option<u64> {
        match &self.0 {
            molgfx::Anchor::World { .. } => None,
            molgfx::Anchor::Selection { structure, .. } => Some(structure.get()),
        }
    }

    /// The query of a `selection` anchor, else `None`.
    #[getter]
    fn target(&self) -> Option<String> {
        match &self.0 {
            molgfx::Anchor::World { .. } => None,
            molgfx::Anchor::Selection { selection, .. } => Some(selection.source().to_owned()),
        }
    }

    fn __repr__(&self) -> String {
        format!("Anchor({})", anchor_repr(&self.0))
    }
}

#[pymethods]
impl PyLabel {
    /// The anchor the label is attached to.
    #[getter]
    fn anchor(&self) -> PyAnchor {
        PyAnchor(self.0.anchor.clone())
    }

    /// The display text.
    #[getter]
    fn text(&self) -> String {
        self.0.text.to_string()
    }

    /// The label color as `(r, g, b)`.
    #[getter]
    fn color(&self) -> (u8, u8, u8) {
        let [red, green, blue, _] = self.0.color.0;
        (red, green, blue)
    }

    fn __repr__(&self) -> String {
        let [red, green, blue, _] = self.0.color.0;
        format!(
            "Label(text={:?}, anchor={}, color=({red}, {green}, {blue}))",
            self.0.text,
            anchor_repr(&self.0.anchor)
        )
    }
}

#[pymethods]
impl PyMeasurement {
    /// `"distance"`, `"angle"` or `"dihedral"`.
    #[getter]
    fn kind(&self) -> &'static str {
        match self.0 {
            molgfx::MeasurementSpec::Distance { .. } => "distance",
            molgfx::MeasurementSpec::Angle { .. } => "angle",
            molgfx::MeasurementSpec::Dihedral { .. } => "dihedral",
        }
    }

    /// The ordered anchors the measurement reads.
    #[getter]
    fn anchors(&self) -> Vec<PyAnchor> {
        let anchors: &[molgfx::Anchor] = match &self.0 {
            molgfx::MeasurementSpec::Distance { anchors } => anchors,
            molgfx::MeasurementSpec::Angle { anchors } => anchors,
            molgfx::MeasurementSpec::Dihedral { anchors } => anchors,
        };
        anchors.iter().cloned().map(PyAnchor).collect()
    }

    fn __repr__(&self) -> String {
        let anchors = self
            .anchors()
            .iter()
            .map(|anchor| anchor_repr(&anchor.0))
            .collect::<Vec<_>>()
            .join(", ");
        format!("Measurement({}, anchors=[{anchors}])", self.kind())
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

#[pyfunction(name = "bind")]
#[pyo3(signature = (*, structure, source, frame_count, time_step=None, time_unit=None))]
fn bind_trajectory(
    structure: &Bound<'_, PyAny>,
    source: &PyDataSource,
    frame_count: u64,
    time_step: Option<f64>,
    time_unit: Option<&str>,
) -> PyResult<PyTrajectory> {
    let (time_step, unit) = crate::time_binding::trajectory_clock(time_step, time_unit)?;
    Ok(PyTrajectory(molgfx::TrajectorySpec {
        structure: molgfx::StructureId::new(crate::id_binding::structure_id(structure)?),
        source: source.0.clone(),
        frame_count,
        time_step,
        time_unit: unit,
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
    interaction_binding::register(&interaction)?;
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
