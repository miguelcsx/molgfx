//! Retained coordinate owners keep binary frame identities stable across pose and trajectory exports.

use crate::{
    bulk_binding::{Column, float_triples},
    scene_binding::PyScene,
};
use pyo3::{exceptions::PyValueError, prelude::*};
use std::collections::{BTreeMap, BTreeSet};

type FrameKey = (usize, usize, u64, u32);
type FrameColumn = (u64, u64, f32, Column);
pub(super) type Interval = (u64, u64, f32);
pub(super) type MotionBindings = (Vec<FrameColumn>, Vec<(u64, u64, u64, f32)>);

struct Frame {
    identity: u64,
    index: u64,
    seconds: f32,
    _owner: molgfx::source::MolecularPose,
    column: Column,
}

#[derive(Default)]
pub(super) struct FrameCache {
    next: u64,
    frames: BTreeMap<FrameKey, Frame>,
}

impl FrameCache {
    pub(super) fn interval(
        &mut self,
        py: Python<'_>,
        pose: &molgfx::source::MolecularPose,
    ) -> PyResult<Option<Interval>> {
        let Some(segment) = pose.trajectory() else {
            return Ok(None);
        };
        let start = self.frame(
            py,
            pose,
            segment.start().index(),
            segment.start().time_seconds(),
            segment.start().positions(),
        )?;
        let end = self.frame(
            py,
            pose,
            segment.end().index(),
            segment.end().time_seconds(),
            segment.end().positions(),
        )?;
        Ok(Some((start, end, segment.sample_seconds())))
    }

    fn frame(
        &mut self,
        py: Python<'_>,
        owner: &molgfx::source::MolecularPose,
        index: u64,
        seconds: f32,
        positions: &[[f32; 3]],
    ) -> PyResult<u64> {
        let key = (
            positions.as_ptr() as usize,
            positions.len(),
            index,
            seconds.to_bits(),
        );
        if let Some(frame) = self.frames.get(&key) {
            return Ok(frame.identity);
        }
        let identity = self
            .next
            .checked_add(1)
            .ok_or_else(|| PyValueError::new_err("coordinate frame identities are exhausted"))?;
        let column = float_triples(py, positions)?;
        let _ = self.frames.insert(
            key,
            Frame {
                identity,
                index,
                seconds,
                _owner: owner.clone(),
                column,
            },
        );
        self.next = identity;
        Ok(identity)
    }
}

#[pymethods]
impl PyScene {
    /// Replaces resident frame owners without authoring an edit or rebuilding molecular topology.
    ///
    /// Needs no `trajectory.bind` descriptor and may be called repeatedly with
    /// different frame pairs, so it is the imperative way to step through a
    /// trajectory of any length. `sample_time` interpolates between `start` and `end`.
    #[pyo3(signature = (*, structure, start, end, sample_time))]
    pub(super) fn set_trajectory_pair(
        slf: &Bound<'_, Self>,
        py: Python<'_>,
        structure: &Bound<'_, PyAny>,
        start: &crate::overlay_binding::PyTrajectoryFrame,
        end: &crate::overlay_binding::PyTrajectoryFrame,
        sample_time: f32,
    ) -> PyResult<()> {
        let identity = crate::id_binding::structure_id(structure)?;
        let identity = molgfx::StructureId::new(identity);
        let same_interval = {
            let mut scene = slf.borrow_mut();
            if scene.pending.is_some() {
                return Err(PyValueError::new_err(
                    "runtime playback cannot change inside a scene transaction",
                ));
            }
            let previous = scene
                .inner
                .molecular_pose(identity)
                .map_err(crate::binding::error)?;
            scene
                .inner
                .set_trajectory_pair(identity, start.0.clone(), end.0.clone(), sample_time)
                .map_err(crate::binding::error)?;
            previous.shares_interval(
                &scene
                    .inner
                    .molecular_pose(identity)
                    .map_err(crate::binding::error)?,
            )
        };
        let event = if same_interval {
            serde_json::to_string(&(identity.get().to_string(), sample_time))
        } else {
            serde_json::to_string(&serde_json::json!({
                "kind": "pair", "structure": identity.get().to_string(), "seconds": sample_time,
            }))
        }
        .map_err(|error| PyValueError::new_err(error.to_string()))?;
        crate::subscription_binding::publish(slf, py, &event, true)
    }

    /// Returns imperative intervals using the same cached frame identities as scientific assets.
    pub(super) fn browser_playback_bindings(
        &mut self,
        py: Python<'_>,
    ) -> PyResult<Vec<(u64, u64, u64, f32)>> {
        let mut intervals = Vec::new();
        for binding in self.inner.playback_bindings() {
            let pose = self
                .inner
                .molecular_pose(binding.structure)
                .map_err(crate::binding::error)?;
            let Some((start, end, seconds)) = self.browser_frames.interval(py, &pose)? else {
                return Err(PyValueError::new_err("playback has no resident interval"));
            };
            intervals.push((binding.structure.get(), start, end, seconds));
        }
        Ok(intervals)
    }
    /// Binds a declared resident interval and publishes its bulk assets after validation.
    ///
    /// The scene must already hold a `molgfx.trajectory.bind(...)` descriptor whose
    /// `source` hash equals `source_hash`; a hash can be bound only once. For
    /// multi-frame playback prefer `molgfx.playback.attach(...)`, which moves the
    /// resident interval with `set_trajectory_pair`.
    #[pyo3(signature = (*, source_hash, start, end, sample_time=None))]
    fn bind_trajectory(
        slf: &Bound<'_, Self>,
        py: Python<'_>,
        source_hash: &str,
        start: &crate::overlay_binding::PyTrajectoryFrame,
        end: &crate::overlay_binding::PyTrajectoryFrame,
        sample_time: Option<f32>,
    ) -> PyResult<()> {
        let binding = molgfx::TrajectoryBinding::new(
            molgfx::schema::DataSource::new(source_hash),
            start.0.clone(),
            end.0.clone(),
        );
        let binding = match sample_time {
            Some(time) => binding.sample_seconds(time),
            None => binding,
        };
        crate::runtime_binding::commit(slf, py, |scene| {
            scene
                .bind_trajectory(binding)
                .map_err(crate::binding::error)
        })
    }

    /// Updates only the resident interpolation sample and publishes a runtime event.
    ///
    /// A pair must already be resident: call `bind_trajectory` (after adding a
    /// `trajectory.bind` descriptor) or `set_trajectory_pair` first. To move to
    /// another segment, call `set_trajectory_pair` again.
    #[pyo3(signature = (*, structure, seconds))]
    fn set_trajectory_time(
        slf: &Bound<'_, Self>,
        py: Python<'_>,
        structure: &Bound<'_, PyAny>,
        seconds: f32,
    ) -> PyResult<()> {
        let identity = crate::id_binding::structure_id(structure)?;
        {
            let mut scene = slf.borrow_mut();
            if scene.pending.is_some() {
                return Err(PyValueError::new_err(
                    "runtime samples cannot be changed inside a scene transaction",
                ));
            }
            scene
                .inner
                .set_trajectory_time(molgfx::StructureId::new(identity), seconds)
                .map_err(|failure| {
                    let missing = failure.to_string().contains("no active trajectory segment");
                    let mapped = crate::binding::error(failure);
                    if missing {
                        let hint = format!(
                            "{mapped}; no frame pair is resident yet. Call set_trajectory_pair(...) \
                             or add a trajectory.bind(...) descriptor followed by \
                             bind_trajectory(...) first (see molgfx.playback.attach for N-frame trajectories)"
                        );
                        PyErr::from_type(mapped.get_type(py), hint)
                    } else {
                        mapped
                    }
                })?;
        }
        let event = serde_json::to_string(&(identity.to_string(), seconds))
            .map_err(|error| PyValueError::new_err(error.to_string()))?;
        crate::subscription_binding::publish(slf, py, &event, true)
    }

    /// Returns shared coordinate columns and resident interval descriptors without authoring an edit.
    pub(super) fn browser_motion_bindings(&mut self, py: Python<'_>) -> PyResult<MotionBindings> {
        let mut seen = BTreeSet::new();
        for binding in self.inner.playback_bindings() {
            let pose = self
                .inner
                .molecular_pose(binding.structure)
                .map_err(crate::binding::error)?;
            if let Some((start, end, _)) = self.browser_frames.interval(py, &pose)? {
                seen.extend([start, end]);
            }
        }
        for (_, binding) in self.inner.bound_hydrogen_bonds() {
            if let Some(pose) = binding.pose()
                && let Some((start, end, _)) = self.browser_frames.interval(py, pose)?
            {
                seen.extend([start, end]);
            }
        }
        for (_, binding) in self.inner.bound_molecular_surfaces() {
            if let Some(pose) = binding.geometry().pose()
                && let Some((start, end, _)) = self.browser_frames.interval(py, pose)?
            {
                seen.extend([start, end]);
            }
        }
        let mut intervals = Vec::new();
        for (id, binding) in self.inner.bound_trajectories() {
            let spec = &self.inner.spec().trajectories[&id];
            let source =
                self.inner.sources().get(&spec.structure).ok_or_else(|| {
                    PyValueError::new_err("bound trajectory has no molecular source")
                })?;
            let pose = binding
                .capture_pose(source)
                .map_err(crate::binding::error)?;
            let Some((start, end, seconds)) = self.browser_frames.interval(py, &pose)? else {
                return Err(PyValueError::new_err(
                    "bound trajectory has no resident coordinate interval",
                ));
            };
            seen.extend([start, end]);
            // The displayed time is authoritative, including uniform-only updates.
            intervals.push((id.get(), start, end, seconds));
        }
        self.browser_frames
            .frames
            .retain(|_, frame| seen.contains(&frame.identity));
        let mut frames: Vec<_> = self
            .browser_frames
            .frames
            .values()
            .map(|frame| {
                (
                    frame.identity,
                    frame.index,
                    frame.seconds,
                    frame.column.clone_ref(py),
                )
            })
            .collect();
        frames.sort_unstable_by_key(|frame| frame.0);
        Ok((frames, intervals))
    }
}
