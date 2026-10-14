//! Shared frame owners preserve exact scientific pose provenance across browser restoration.
use super::{WebScene, javascript_error};
use std::sync::Arc;
use wasm_bindgen::prelude::*;

impl WebScene {
    pub(super) fn prune_motion_bindings(&mut self, spec: &molgfx::SceneSpec) {
        self.playback_bindings
            .retain(|structure, _| spec.structures.contains_key(structure));
        self.trajectory_bindings.retain(|hash, _| {
            spec.trajectories
                .values()
                .any(|trajectory| trajectory.source.content_hash == *hash)
        });
        self.hydrogen_bond_bindings.retain(|hash, _| {
            spec.interactions.values().any(|interaction| {
                matches!(interaction, molgfx::InteractionSpec::HydrogenBonds(value)
                    if value.source.content_hash == *hash)
            })
        });
    }

    pub(super) fn prune_coordinate_frames(&mut self) {
        let mut live = std::collections::HashSet::new();
        for binding in self.playback_bindings.values() {
            for frame in [&binding.start, &binding.end] {
                live.insert((
                    frame.positions().as_ptr() as usize,
                    frame.index(),
                    frame.time_seconds().to_bits(),
                ));
            }
        }
        for binding in self.trajectory_bindings.values() {
            let (start, end) = binding.frames();
            for frame in [start, end] {
                live.insert((
                    frame.positions().as_ptr() as usize,
                    frame.index(),
                    frame.time_seconds().to_bits(),
                ));
            }
        }
        for (_, binding) in self.molecular_surface_bindings.values() {
            if let Some(segment) = binding
                .geometry()
                .pose()
                .and_then(molgfx::source::MolecularPose::trajectory)
            {
                for frame in [segment.start(), segment.end()] {
                    live.insert((
                        frame.positions().as_ptr() as usize,
                        frame.index(),
                        frame.time_seconds().to_bits(),
                    ));
                }
            }
        }
        for binding in self.hydrogen_bond_bindings.values() {
            if let Some(segment) = binding
                .pose()
                .and_then(molgfx::source::MolecularPose::trajectory)
            {
                for frame in [segment.start(), segment.end()] {
                    live.insert((
                        frame.positions().as_ptr() as usize,
                        frame.index(),
                        frame.time_seconds().to_bits(),
                    ));
                }
            }
        }
        self.coordinate_frames.retain(|_, frame| {
            live.contains(&(
                frame.positions().as_ptr() as usize,
                frame.index(),
                frame.time_seconds().to_bits(),
            ))
        });
    }

    pub(super) fn resident_binding(
        &self,
        source: molgfx::schema::DataSource,
        start: u64,
        end: u64,
        seconds: f64,
    ) -> Result<molgfx::TrajectoryBinding, JsError> {
        let start = self
            .coordinate_frames
            .get(&start)
            .ok_or_else(|| JsError::new("resident interval has no start frame"))?;
        let end = self
            .coordinate_frames
            .get(&end)
            .ok_or_else(|| JsError::new("resident interval has no end frame"))?;
        Ok(
            molgfx::TrajectoryBinding::new(source, start.clone(), end.clone())
                .sample_seconds(seconds),
        )
    }
}

#[wasm_bindgen(js_class = Scene)]
impl WebScene {
    /// Stages one topology-aligned coordinate column shared by all references to this identity.
    ///
    /// # Errors
    /// Refuses resolved scenes, duplicate identities, malformed or nonfinite coordinates.
    #[wasm_bindgen(js_name = restoreCoordinateFrame)]
    pub fn restore_coordinate_frame(
        &mut self,
        identity: u64,
        index: u64,
        seconds: f64,
        positions: &[f32],
    ) -> Result<(), JsError> {
        self.require_unresolved()?;
        if positions.is_empty()
            || !positions.len().is_multiple_of(3)
            || !seconds.is_finite()
            || positions.iter().any(|value| !value.is_finite())
        {
            return Err(JsError::new(
                "coordinate frames require finite time and complete finite atom triples",
            ));
        }
        if self.coordinate_frames.contains_key(&identity) {
            return Err(JsError::new("coordinate frame is already restored"));
        }
        let frame =
            molgfx::TrajectoryFrame::new(index, seconds, Arc::from(positions.as_chunks::<3>().0));
        let _ = self.coordinate_frames.insert(identity, frame);
        Ok(())
    }

    /// Stages an authored trajectory interval without recording an edit or decoding its frames twice.
    ///
    /// # Errors
    /// Refuses resolved scenes, missing frames, unknown trajectories or duplicate content identities.
    #[wasm_bindgen(js_name = restoreTrajectory)]
    pub fn restore_trajectory(
        &mut self,
        identity: u64,
        start: u64,
        end: u64,
        seconds: f64,
    ) -> Result<(), JsError> {
        self.require_unresolved()?;
        let spec = self
            .spec
            .trajectories
            .get(&molgfx::TrajectoryId::new(identity))
            .ok_or_else(|| JsError::new("unknown trajectory identity"))?;
        let hash = spec.source.content_hash.clone();
        if self.trajectory_bindings.contains_key(&hash) {
            return Err(JsError::new("trajectory is already restored"));
        }
        let binding = self.resident_binding(spec.source.clone(), start, end, seconds)?;
        let _ = self.trajectory_bindings.insert(hash, binding);
        Ok(())
    }

    /// Updates the resident interpolation uniform without recording a command or uploading coordinates.
    ///
    /// # Errors
    /// Refuses unresolved scenes and times outside the resident interval.
    #[wasm_bindgen(js_name = setTrajectoryTime)]
    pub fn set_trajectory_time(
        &mut self,
        structure: u64,
        sample_seconds: f64,
    ) -> Result<(), JsError> {
        let scene = self
            .resolved
            .as_mut()
            .ok_or_else(|| JsError::new("scene must be resolved before trajectory updates"))?;
        scene
            .set_trajectory_time(molgfx::StructureId::new(structure), sample_seconds)
            .map_err(javascript_error)?;
        for binding in scene.playback_bindings() {
            let _ = self.playback_bindings.insert(binding.structure, binding);
        }
        for (id, binding) in scene.bound_trajectories() {
            if let Some(retained) = self
                .trajectory_bindings
                .get_mut(&scene.spec().trajectories[&id].source.content_hash)
            {
                *retained = binding.clone();
            }
        }
        Ok(())
    }
}
