//! Python facade for allocation-free multi-rate timeline synchronization.

use crate::core::{
    PyAttributeHandle, PyInstanceBatchHandle, PyPointBatchHandle, PyScene, PyStructureHandle,
    PyTimelineTrackHandle,
};
use crate::error::core;
use crate::math::PyCamera;
use numpy::{PyReadonlyArray1, PyReadonlyArray2, PyUntypedArrayMethods};
use pyo3::prelude::*;
use std::sync::Arc;

#[pyclass(name = "CameraEasing", frozen, eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PyCameraEasing {
    Linear,
    SmoothStep,
}

impl From<PyCameraEasing> for molgfx::CameraEasing {
    fn from(value: PyCameraEasing) -> Self {
        match value {
            PyCameraEasing::Linear => Self::Linear,
            PyCameraEasing::SmoothStep => Self::SmoothStep,
        }
    }
}

#[pyclass(name = "CameraKeyframe", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyCameraKeyframe(molgfx::CameraKeyframe);

#[pymethods]
impl PyCameraKeyframe {
    #[new]
    fn new(time_seconds: f64, camera: PyCamera) -> PyResult<Self> {
        core(molgfx::CameraKeyframe::new(time_seconds, camera.inner)).map(Self)
    }

    #[getter]
    fn time_seconds(&self) -> f64 {
        self.0.time_seconds()
    }

    #[getter]
    fn camera(&self) -> PyCamera {
        PyCamera {
            inner: self.0.camera(),
        }
    }
}

#[pyclass(name = "CameraPath", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyCameraPath(molgfx::CameraPath);

#[pymethods]
impl PyCameraPath {
    #[new]
    #[pyo3(signature = (keyframes, easing=PyCameraEasing::SmoothStep))]
    fn new(keyframes: Vec<PyCameraKeyframe>, easing: PyCameraEasing) -> PyResult<Self> {
        let keyframes = keyframes
            .into_iter()
            .map(|value| value.0)
            .collect::<Vec<_>>();
        core(molgfx::CameraPath::new(
            Arc::from(keyframes.into_boxed_slice()),
            easing.into(),
        ))
        .map(Self)
    }

    fn sample(&self, time_seconds: f64) -> Option<PyCamera> {
        self.0.sample(time_seconds).map(|inner| PyCamera { inner })
    }

    #[getter]
    fn keyframes(&self) -> Vec<PyCameraKeyframe> {
        self.0
            .keyframes()
            .iter()
            .copied()
            .map(PyCameraKeyframe)
            .collect()
    }

    #[getter]
    fn range(&self) -> (f64, f64) {
        let [start, end] = self.0.range();
        (start, end)
    }
}

#[pyclass(name = "CameraBookmark", frozen, from_py_object)]
#[derive(Clone, Debug)]
pub(crate) struct PyCameraBookmark(molgfx::CameraBookmark);

#[pymethods]
impl PyCameraBookmark {
    #[new]
    fn new(label: String, time_seconds: f64, camera: PyCamera) -> PyResult<Self> {
        core(molgfx::CameraBookmark::new(
            label,
            time_seconds,
            camera.inner,
        ))
        .map(Self)
    }

    #[staticmethod]
    fn from_json(source: &str) -> PyResult<Self> {
        core(molgfx::CameraBookmark::from_json(source)).map(Self)
    }

    fn to_json(&self) -> PyResult<String> {
        core(self.0.to_json())
    }

    fn restore(
        &self,
        mut timeline: PyRefMut<'_, PyTimeline>,
        mut scene: PyRefMut<'_, PyScene>,
        mut camera: PyRefMut<'_, PyCamera>,
    ) -> PyResult<()> {
        core(
            self.0
                .restore(&mut timeline.0, &mut scene.inner, &mut camera.inner),
        )
    }

    #[getter]
    fn label(&self) -> &str {
        self.0.label()
    }

    #[getter]
    fn time_seconds(&self) -> f64 {
        self.0.time_seconds()
    }

    #[getter]
    fn camera(&self) -> PyCamera {
        PyCamera {
            inner: self.0.camera(),
        }
    }
}

#[pyclass(name = "PlaybackMode", frozen, eq, eq_int, from_py_object)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PyPlaybackMode {
    Clamp,
    Loop,
    PingPong,
}

impl From<PyPlaybackMode> for molgfx::PlaybackMode {
    fn from(value: PyPlaybackMode) -> Self {
        match value {
            PyPlaybackMode::Clamp => Self::Clamp,
            PyPlaybackMode::Loop => Self::Loop,
            PyPlaybackMode::PingPong => Self::PingPong,
        }
    }
}

#[pyclass(name = "TimeWarp", frozen, from_py_object)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct PyTimeWarp(pub(crate) molgfx::TimeWarp);

#[pymethods]
impl PyTimeWarp {
    #[new]
    fn new(
        global_origin: f64,
        local_origin: f64,
        rate: f64,
        range: (f64, f64),
        playback: PyPlaybackMode,
    ) -> PyResult<Self> {
        core(molgfx::TimeWarp::new(
            global_origin,
            local_origin,
            rate,
            [range.0, range.1],
            playback.into(),
        ))
        .map(Self)
    }

    fn sample(&self, global_seconds: f64) -> Option<f64> {
        self.0.sample(global_seconds)
    }

    fn phase(&self, global_seconds: f64) -> Option<f32> {
        self.0.phase(global_seconds)
    }

    #[getter]
    fn range(&self) -> (f64, f64) {
        let [start, end] = self.0.range();
        (start, end)
    }
}

#[pyclass(name = "Timeline")]
#[derive(Debug)]
pub(crate) struct PyTimeline(pub(crate) molgfx::Timeline);

#[pymethods]
impl PyTimeline {
    #[new]
    fn new() -> Self {
        Self(molgfx::Timeline::new())
    }

    fn bind_trajectory(
        &mut self,
        scene: PyRef<'_, PyScene>,
        structure: PyStructureHandle,
        warp: PyTimeWarp,
    ) -> PyResult<PyTimelineTrackHandle> {
        core(self.0.bind_trajectory(&scene.inner, structure.0, warp.0)).map(Into::into)
    }

    fn bind_bond_topology(
        &mut self,
        scene: PyRef<'_, PyScene>,
        structure: PyStructureHandle,
        warp: PyTimeWarp,
    ) -> PyResult<PyTimelineTrackHandle> {
        core(self.0.bind_bond_topology(&scene.inner, structure.0, warp.0)).map(Into::into)
    }

    fn bind_points_from_numpy(
        &mut self,
        mut scene: PyRefMut<'_, PyScene>,
        batch: PyPointBatchHandle,
        start: PyReadonlyArray2<'_, f32>,
        end: PyReadonlyArray2<'_, f32>,
        warp: PyTimeWarp,
    ) -> PyResult<PyTimelineTrackHandle> {
        let start = vector_frames(&start)?;
        let end = vector_frames(&end)?;
        core(self.0.bind_points(
            &mut scene.inner,
            batch.0,
            Arc::from(start),
            Arc::from(end),
            warp.0,
        ))
        .map(Into::into)
    }

    #[pyo3(signature = (scene, batch, start_translations, start_orientations, start_scales, end_translations, end_orientations, end_scales, warp))]
    fn bind_instances_from_numpy(
        &mut self,
        mut scene: PyRefMut<'_, PyScene>,
        batch: PyInstanceBatchHandle,
        start_translations: PyReadonlyArray2<'_, f32>,
        start_orientations: PyReadonlyArray2<'_, f32>,
        start_scales: PyReadonlyArray1<'_, f32>,
        end_translations: PyReadonlyArray2<'_, f32>,
        end_orientations: PyReadonlyArray2<'_, f32>,
        end_scales: PyReadonlyArray1<'_, f32>,
        warp: PyTimeWarp,
    ) -> PyResult<PyTimelineTrackHandle> {
        let start = rigid_frames(&start_translations, &start_orientations, &start_scales)?;
        let end = rigid_frames(&end_translations, &end_orientations, &end_scales)?;
        core(self.0.bind_instances(
            &mut scene.inner,
            batch.0,
            Arc::from(start),
            Arc::from(end),
            warp.0,
        ))
        .map(Into::into)
    }

    fn bind_scalar_attribute_from_numpy(
        &mut self,
        mut scene: PyRefMut<'_, PyScene>,
        attribute: PyAttributeHandle,
        start: PyReadonlyArray1<'_, f32>,
        end: PyReadonlyArray1<'_, f32>,
        warp: PyTimeWarp,
    ) -> PyResult<PyTimelineTrackHandle> {
        let start = start
            .as_slice()
            .map_err(|_| pyo3::exceptions::PyValueError::new_err("start must be contiguous"))?;
        let end = end
            .as_slice()
            .map_err(|_| pyo3::exceptions::PyValueError::new_err("end must be contiguous"))?;
        core(self.0.bind_attribute(
            &mut scene.inner,
            attribute.0,
            molgfx::AttributeValues::Scalar(Arc::from(start)),
            molgfx::AttributeValues::Scalar(Arc::from(end)),
            warp.0,
        ))
        .map(Into::into)
    }

    fn bind_vector_attribute_from_numpy(
        &mut self,
        mut scene: PyRefMut<'_, PyScene>,
        attribute: PyAttributeHandle,
        start: PyReadonlyArray2<'_, f32>,
        end: PyReadonlyArray2<'_, f32>,
        warp: PyTimeWarp,
    ) -> PyResult<PyTimelineTrackHandle> {
        let start = vector_frames(&start)?;
        let end = vector_frames(&end)?;
        core(self.0.bind_attribute(
            &mut scene.inner,
            attribute.0,
            molgfx::AttributeValues::Vector(Arc::from(start)),
            molgfx::AttributeValues::Vector(Arc::from(end)),
            warp.0,
        ))
        .map(Into::into)
    }

    fn remove(&mut self, mut scene: PyRefMut<'_, PyScene>, handle: PyTimelineTrackHandle) -> bool {
        self.0.unbind(&mut scene.inner, handle.0)
    }

    fn apply(&mut self, mut scene: PyRefMut<'_, PyScene>, global_seconds: f64) -> PyResult<()> {
        core(self.0.apply(&mut scene.inner, global_seconds))
    }

    #[getter]
    fn time_seconds(&self) -> f64 {
        self.0.time_seconds()
    }

    fn __len__(&self) -> usize {
        self.0.len()
    }

    fn __bool__(&self) -> bool {
        !self.0.is_empty()
    }
}

fn rigid_frames(
    translations: &PyReadonlyArray2<'_, f32>,
    orientations: &PyReadonlyArray2<'_, f32>,
    scales: &PyReadonlyArray1<'_, f32>,
) -> PyResult<Vec<molgfx::RigidInstance>> {
    let translation_shape = translations.shape();
    if translation_shape.len() != 2 || translation_shape[1] != 3 {
        return Err(pyo3::exceptions::PyValueError::new_err(
            "timeline translations must have shape (N,3)",
        ));
    }
    let count = translation_shape[0];
    if orientations.shape() != [count, 4] || scales.shape() != [count] {
        return Err(pyo3::exceptions::PyValueError::new_err(
            "timeline transform columns must have shapes (N,3), (N,4), and (N,)",
        ));
    }
    let translations = translations
        .as_slice()
        .map_err(|_| pyo3::exceptions::PyValueError::new_err("translations must be contiguous"))?;
    let orientations = orientations
        .as_slice()
        .map_err(|_| pyo3::exceptions::PyValueError::new_err("orientations must be contiguous"))?;
    let scales = scales
        .as_slice()
        .map_err(|_| pyo3::exceptions::PyValueError::new_err("scales must be contiguous"))?;
    let mut frames = Vec::with_capacity(count);
    for row in 0..count {
        frames.push(core(molgfx::RigidInstance::new(
            molgfx::Vec3::from_array([
                translations[row * 3],
                translations[row * 3 + 1],
                translations[row * 3 + 2],
            ]),
            molgfx::Quat::from_array([
                orientations[row * 4],
                orientations[row * 4 + 1],
                orientations[row * 4 + 2],
                orientations[row * 4 + 3],
            ]),
            scales[row],
        ))?);
    }
    Ok(frames)
}

fn vector_frames(values: &PyReadonlyArray2<'_, f32>) -> PyResult<Vec<[f32; 3]>> {
    let shape = values.shape();
    if shape.len() != 2 || shape[1] != 3 {
        return Err(pyo3::exceptions::PyValueError::new_err(
            "attribute vector frames must have shape (N,3)",
        ));
    }
    let values = values.as_slice().map_err(|_| {
        pyo3::exceptions::PyValueError::new_err("attribute vector frames must be contiguous")
    })?;
    Ok(values
        .chunks_exact(3)
        .map(|row| [row[0], row[1], row[2]])
        .collect())
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyCameraEasing>()?;
    module.add_class::<PyCameraKeyframe>()?;
    module.add_class::<PyCameraPath>()?;
    module.add_class::<PyCameraBookmark>()?;
    module.add_class::<PyPlaybackMode>()?;
    module.add_class::<PyTimeWarp>()?;
    module.add_class::<PyTimeline>()
}
