//! `molgfx.Timeline`: independently warped trajectory tracks on one playback clock.

use crate::id_binding::PyStructureId;
use crate::scene_binding::PyScene;
use crate::time_binding::{parse_unit, unit_member};
use molgfx::StructureId;
use molgfx::time::{SimTime, TimeUnit};
use molgfx::timeline::{PlaybackMode, TimeWarp, Timeline, TimelineTrackHandle};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyList, PyTuple};

/// One structure's track on a `Timeline`; hand it back to `Timeline.reading`.
#[pyclass(name = "TimelineTrack", frozen, skip_from_py_object)]
struct PyTimelineTrack {
    handle: TimelineTrackHandle,
    structure: u64,
}

#[pymethods]
impl PyTimelineTrack {
    /// The structure this track moves.
    #[getter]
    const fn structure(&self) -> PyStructureId {
        PyStructureId(self.structure)
    }
}

/// Several trajectory tracks, each with its own pace, driven by one clock.
#[pyclass(name = "Timeline")]
struct PyTimeline {
    scene: Py<PyScene>,
    inner: Timeline,
    structures: Vec<u64>,
}

fn mode(name: Option<&str>) -> PyResult<PlaybackMode> {
    match name {
        None | Some("clamp") => Ok(PlaybackMode::Clamp),
        Some("loop") => Ok(PlaybackMode::Loop),
        Some("ping_pong") => Ok(PlaybackMode::PingPong),
        Some(other) => Err(PyValueError::new_err(format!(
            "unknown playback mode `{other}`"
        ))),
    }
}

/// The physical interval `[start, end]` given in `unit`.
fn interval((start, end): (f64, f64), unit: TimeUnit) -> PyResult<[SimTime; 2]> {
    let seconds = |value| {
        SimTime::from_unit(value, unit).map_err(|error| PyValueError::new_err(error.to_string()))
    };
    Ok([seconds(start)?, seconds(end)?])
}

impl PyTimeline {
    fn bind(
        &mut self,
        structure: &Bound<'_, PyAny>,
        warp: TimeWarp,
        display_unit: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyTimelineTrack> {
        let warp = match display_unit {
            Some(unit) => warp.with_unit(parse_unit(unit)?),
            None => warp,
        };
        let py = structure.py();
        let structure = crate::id_binding::structure_id(structure)?;
        let handle = self
            .scene
            .bind(py)
            .borrow()
            .inner
            .bind_timeline_trajectory(&mut self.inner, StructureId::new(structure), warp)
            .map_err(crate::binding::error)?;
        self.structures.push(structure);
        Ok(PyTimelineTrack { handle, structure })
    }
}

#[pymethods]
impl PyTimeline {
    #[new]
    fn new(scene: &Bound<'_, PyScene>) -> Self {
        Self {
            scene: scene.clone().unbind(),
            inner: Timeline::new(),
            structures: Vec::new(),
        }
    }

    /// Plays `range = (start, end)` (in `unit`) of a structure's resident frame pair once
    /// across `seconds` of playback.
    ///
    /// The readout unit defaults to the one that fits the interval.
    #[pyo3(signature = (structure, *, range, unit, seconds, mode=None, display_unit=None))]
    fn fit_track(
        &mut self,
        structure: &Bound<'_, PyAny>,
        range: (f64, f64),
        unit: &Bound<'_, PyAny>,
        seconds: f64,
        mode: Option<&str>,
        display_unit: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyTimelineTrack> {
        let range = interval(range, parse_unit(unit)?)?;
        let warp = TimeWarp::fit(range, seconds, self::mode(mode)?)
            .map_err(|error| PyValueError::new_err(error.to_string()))?;
        self.bind(structure, warp, display_unit)
    }

    /// Plays `range = (start, end)` (in `unit`) at `per_second` of physical time per
    /// second of playback, for example one nanosecond per second.
    #[pyo3(signature = (structure, *, range, unit, per_second, mode=None, display_unit=None))]
    fn dilate_track(
        &mut self,
        structure: &Bound<'_, PyAny>,
        range: (f64, f64),
        unit: &Bound<'_, PyAny>,
        per_second: f64,
        mode: Option<&str>,
        display_unit: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<PyTimelineTrack> {
        let unit = parse_unit(unit)?;
        let range = interval(range, unit)?;
        let pace = SimTime::from_unit(per_second, unit)
            .map_err(|error| PyValueError::new_err(error.to_string()))?;
        let warp = TimeWarp::dilation(pace, range, self::mode(mode)?)
            .map_err(|error| PyValueError::new_err(error.to_string()))?;
        self.bind(structure, warp, display_unit)
    }

    /// Moves every track to `playback_seconds`, atomically.
    ///
    /// Reuses the resident frame pairs: no positions are copied and nothing is
    /// allocated per call beyond the runtime events subscribers receive.
    fn apply(slf: &Bound<'_, Self>, py: Python<'_>, playback_seconds: f64) -> PyResult<()> {
        let events = {
            let mut this = slf.borrow_mut();
            let scene_object = this.scene.clone_ref(py);
            let mut scene = scene_object.bind(py).borrow_mut();
            scene.reject_pending("runtime playback cannot change inside a scene transaction")?;
            scene
                .inner
                .apply_timeline(&mut this.inner, playback_seconds)
                .map_err(crate::binding::error)?;
            if scene.runtime_subscribers.is_empty() {
                Vec::new()
            } else {
                this.inner
                    .readings()
                    .zip(&this.structures)
                    .map(|((_, reading), structure)| {
                        serde_json::to_string(&(structure.to_string(), reading.time.seconds()))
                            .map_err(|error| PyValueError::new_err(error.to_string()))
                    })
                    .collect::<PyResult<Vec<_>>>()?
            }
        };
        let scene = slf.borrow().scene.clone_ref(py);
        for event in events {
            crate::subscription_binding::publish(scene.bind(py), py, &event, true)?;
        }
        Ok(())
    }

    /// Every track's physical time as `(value, unit)`, in the track's own
    /// display unit, in the order the tracks were added.
    fn readings<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyList>> {
        let list = PyList::empty(py);
        for (_, reading) in self.inner.readings() {
            list.append(reading_pair(py, reading.display_value(), reading.unit)?)?;
        }
        Ok(list)
    }

    /// One track's `(value, unit)`.
    fn reading<'py>(
        &self,
        py: Python<'py>,
        track: &PyTimelineTrack,
    ) -> PyResult<Bound<'py, PyTuple>> {
        let reading = self
            .inner
            .reading(track.handle)
            .ok_or_else(|| PyValueError::new_err("the track does not belong to this timeline"))?;
        reading_pair(py, reading.display_value(), reading.unit)
    }

    /// The playback time most recently applied, in seconds.
    #[getter]
    const fn seconds(&self) -> f64 {
        self.inner.time_seconds()
    }

    fn __len__(&self) -> usize {
        self.inner.len()
    }
}

fn reading_pair(py: Python<'_>, value: f64, unit: TimeUnit) -> PyResult<Bound<'_, PyTuple>> {
    PyTuple::new(
        py,
        [value.into_pyobject(py)?.into_any(), unit_member(py, unit)?],
    )
}

pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyTimeline>()?;
    module.add_class::<PyTimelineTrack>()
}
