//! `molgfx.playback`: an N-frame trajectory stepped through a scene's resident pair.

use crate::id_binding::PyStructureId;
use crate::overlay_binding::PyTrajectoryFrame;
use crate::render_binding::{PyImage, PyRenderer};
use crate::scene_binding::PyScene;
use molgfx::playback::{FrameCache, Timeline};
use pyo3::exceptions::{PyIndexError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyModule, PyTuple};

/// A scene bound to explicit frames; `seek` moves the resident pair.
#[pyclass(name = "Playback")]
struct PyPlayback {
    scene: Py<PyScene>,
    structure: Py<PyAny>,
    positions: Py<PyAny>,
    timeline: Timeline,
    cache: FrameCache,
    segment: Option<usize>,
    at: Option<f32>,
    pair: Option<(PyTrajectoryFrame, PyTrajectoryFrame)>,
}

impl PyPlayback {
    fn frame(&mut self, py: Python<'_>, index: usize) -> PyResult<PyTrajectoryFrame> {
        let time = self
            .timeline
            .time(index)
            .ok_or_else(|| PyIndexError::new_err(format!("no frame {index}")))?;
        let positions = &self.positions;
        self.cache
            .get_or_convert(index, || {
                let atoms = positions.bind(py).get_item(index)?;
                PyTrajectoryFrame::new(index as u64, f64::from(time), &atoms).map(|frame| frame.0)
            })
            .map(PyTrajectoryFrame)
    }

    fn move_to(&mut self, py: Python<'_>, time: f32) -> PyResult<()> {
        let position = self.timeline.locate(time);
        if self.at == Some(position.time) && self.pair.is_some() {
            return Ok(());
        }
        if self.segment != Some(position.segment) || self.pair.is_none() {
            let start = self.frame(py, position.segment)?;
            let end = self.frame(py, position.segment + 1)?;
            self.pair = Some((start, end));
        }
        if let Some((start, end)) = &self.pair {
            PyScene::set_trajectory_pair(
                self.scene.bind(py),
                py,
                self.structure.bind(py),
                start,
                end,
                f64::from(position.time),
            )?;
        }
        self.segment = Some(position.segment);
        self.at = Some(position.time);
        Ok(())
    }
}

#[pymethods]
impl PyPlayback {
    #[getter]
    fn scene(&self, py: Python<'_>) -> Py<PyScene> {
        self.scene.clone_ref(py)
    }

    #[getter]
    fn structure(&self, py: Python<'_>) -> Py<PyAny> {
        self.structure.clone_ref(py)
    }

    #[getter]
    fn positions(&self, py: Python<'_>) -> Py<PyAny> {
        self.positions.clone_ref(py)
    }

    #[getter]
    fn times<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyTuple>> {
        PyTuple::new(py, self.timeline.times())
    }

    /// Make the scene show the interpolated state at `time`.
    ///
    /// `time` is clamped to the first and last frame time. Seeking to the time
    /// already shown does nothing, and seeking within the current segment
    /// reuses its frame handles.
    fn seek(&mut self, py: Python<'_>, time: f32) -> PyResult<()> {
        self.move_to(py, time)
    }

    /// Render one image per entry of `times`, seeking before each.
    #[pyo3(signature = (renderer, *, times, size))]
    fn render(
        slf: &Bound<'_, Self>,
        renderer: &Bound<'_, PyRenderer>,
        times: &Bound<'_, PyAny>,
        size: (u32, u32),
    ) -> PyResult<Vec<PyImage>> {
        let py = slf.py();
        let mut images = Vec::new();
        for time in times.try_iter()? {
            let time: f32 = time?.extract()?;
            slf.borrow_mut().move_to(py, time)?;
            let scene = slf.borrow().scene.clone_ref(py);
            let image = renderer
                .borrow_mut()
                .render_image(py, &scene.bind(py).borrow(), size)?;
            images.push(image);
        }
        Ok(images)
    }
}

/// Bind frames to a structure and return a `Playback` positioned at frame 0.
///
/// `positions` is `(frames, atoms, 3)` in the structure's atom order and
/// `times` has one strictly ascending entry per frame. At least two frames are
/// needed.
#[pyfunction]
#[pyo3(signature = (scene, positions, times, *, structure=None))]
fn attach(
    scene: &Bound<'_, PyScene>,
    positions: &Bound<'_, PyAny>,
    times: &Bound<'_, PyAny>,
    structure: Option<&Bound<'_, PyAny>>,
) -> PyResult<PyPlayback> {
    let py = scene.py();
    let stamps = times
        .try_iter()?
        .map(|time| time?.extract::<f32>())
        .collect::<PyResult<Vec<_>>>()?;
    let timeline = Timeline::new(positions.len()?, stamps)
        .map_err(|failure| PyValueError::new_err(failure.to_string()))?;
    let &[first, ..] = timeline.times() else {
        return Err(PyValueError::new_err(
            "a playback needs at least two frames",
        ));
    };
    let structure = match structure {
        Some(structure) => structure.clone().unbind(),
        None => Py::new(py, PyStructureId(scene.borrow().structure_id))?.into_any(),
    };
    let mut play = PyPlayback {
        scene: scene.clone().unbind(),
        structure,
        positions: positions.clone().unbind(),
        timeline,
        cache: FrameCache::default(),
        segment: None,
        at: None,
        pair: None,
    };
    play.move_to(py, first)?;
    Ok(play)
}

/// `attach` for a molframe `Trajectory` (its `positions` and `times`).
#[pyfunction]
#[pyo3(signature = (scene, source, *, structure=None))]
fn attach_trajectory(
    scene: &Bound<'_, PyScene>,
    source: &Bound<'_, PyAny>,
    structure: Option<&Bound<'_, PyAny>>,
) -> PyResult<PyPlayback> {
    attach(
        scene,
        &source.getattr("positions")?,
        &source.getattr("times")?,
        structure,
    )
}

pub(super) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    let playback = PyModule::new(module.py(), "playback")?;
    module.add_class::<PyPlayback>()?;
    playback.add_class::<PyPlayback>()?;
    playback.add_function(wrap_pyfunction!(attach, &playback)?)?;
    playback.add_function(wrap_pyfunction!(attach_trajectory, &playback)?)?;
    module.add_submodule(&playback)
}
