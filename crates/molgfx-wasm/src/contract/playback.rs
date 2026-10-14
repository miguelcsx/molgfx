//! Atomic runtime interval swaps keep coordinate owners and the authored revision.

use super::{WebScene, javascript_error};
use std::sync::Arc;
use wasm_bindgen::prelude::*;

#[derive(serde::Deserialize)]
struct FrameReference {
    identity: String,
    index: String,
    seconds: f32,
}

#[derive(serde::Deserialize)]
struct PlaybackMetadata {
    start: FrameReference,
    end: FrameReference,
    seconds: f32,
}

impl WebScene {
    fn playback_frame(
        &self,
        reference: &FrameReference,
        positions: &[f32],
    ) -> Result<(u64, molgfx::TrajectoryFrame), JsError> {
        let identity = reference.identity.parse().map_err(javascript_error)?;
        let index = reference.index.parse().map_err(javascript_error)?;
        if let Some(frame) = self.coordinate_frames.get(&identity) {
            if frame.index() != index
                || frame.time_seconds().to_bits() != f64::from(reference.seconds).to_bits()
                || (!positions.is_empty()
                    && (positions.len() != frame.positions().len() * 3
                        || positions
                            .iter()
                            .zip(frame.positions().iter().flatten())
                            .any(|(a, b)| a.to_bits() != b.to_bits())))
            {
                return Err(JsError::new(
                    "coordinate frame identity belongs to different retained input",
                ));
            }
            return Ok((identity, frame.clone()));
        }
        if positions.is_empty()
            || !positions.len().is_multiple_of(3)
            || !reference.seconds.is_finite()
            || positions.iter().any(|value| !value.is_finite())
        {
            return Err(JsError::new(
                "new coordinate frame requires finite complete atom triples",
            ));
        }
        Ok((
            identity,
            molgfx::TrajectoryFrame::new(
                index,
                f64::from(reference.seconds),
                Arc::from(positions.as_chunks::<3>().0),
            ),
        ))
    }
}

#[wasm_bindgen(js_class = Scene)]
impl WebScene {
    /// Whether this exact wire identity already has retained coordinate storage.
    #[wasm_bindgen(js_name = hasCoordinateFrame)]
    pub fn has_coordinate_frame(&self, identity: u64) -> bool {
        self.coordinate_frames.contains_key(&identity)
    }

    /// Atomically swaps an interval; already retained frames need no repeated coordinate payload.
    ///
    /// # Errors
    /// Refuses unresolved scenes, inconsistent frame identities and invalid topology or time.
    #[wasm_bindgen(js_name = setTrajectoryPair)]
    pub fn set_trajectory_pair(
        &mut self,
        structure: u64,
        metadata: &str,
        start_positions: &[f32],
        end_positions: &[f32],
    ) -> Result<(), JsError> {
        let metadata: PlaybackMetadata =
            serde_json::from_str(metadata).map_err(javascript_error)?;
        let (start_id, start) = self.playback_frame(&metadata.start, start_positions)?;
        let (end_id, end) = self.playback_frame(&metadata.end, end_positions)?;
        if start_id == end_id {
            return Err(JsError::new(
                "playback requires two distinct frame identities",
            ));
        }
        let structure = molgfx::StructureId::new(structure);
        self.resolved
            .as_mut()
            .ok_or_else(|| JsError::new("resolve the scene before runtime playback"))?
            .set_trajectory_pair(
                structure,
                start.clone(),
                end.clone(),
                f64::from(metadata.seconds),
            )
            .map_err(javascript_error)?;
        let _ = self.coordinate_frames.insert(start_id, start.clone());
        let _ = self.coordinate_frames.insert(end_id, end.clone());
        let _ = self.playback_bindings.insert(
            structure,
            molgfx::PlaybackBinding {
                structure,
                start,
                end,
                sample_seconds: f64::from(metadata.seconds),
            },
        );
        self.prune_coordinate_frames();
        Ok(())
    }

    /// Stages an imperative interval for revision-preserving restoration.
    ///
    /// # Errors
    /// Refuses resolved scenes, unknown molecules, missing frames and duplicate intervals.
    #[wasm_bindgen(js_name = restorePlayback)]
    pub fn restore_playback(
        &mut self,
        structure: u64,
        start: u64,
        end: u64,
        seconds: f32,
    ) -> Result<(), JsError> {
        self.require_unresolved()?;
        let structure = molgfx::StructureId::new(structure);
        if !self.structures.contains_key(&structure)
            || self.playback_bindings.contains_key(&structure)
        {
            return Err(JsError::new(
                "playback requires one bound molecular placement and one interval",
            ));
        }
        let binding = self.resident_binding(
            molgfx::schema::DataSource::new("playback"),
            start,
            end,
            seconds,
        )?;
        let (start, end) = binding.frames();
        let _ = self.playback_bindings.insert(
            structure,
            molgfx::PlaybackBinding {
                structure,
                start: start.clone(),
                end: end.clone(),
                sample_seconds: f64::from(seconds),
            },
        );
        Ok(())
    }
}
