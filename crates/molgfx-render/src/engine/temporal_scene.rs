//! History compatibility for both presented frames and offscreen exposures.

use super::Engine;
use molgfx_core::Scene;
use molgfx_gpu::Device;

/// Exact scene revisions that change what an accumulated pixel represents.
/// Coordinate timelines and the presentation clock deliberately stay out:
/// their motion is reprojected rather than discarded on every frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TemporalSceneKey {
    identity: u64,
    revisions: [u64; 16],
}

impl TemporalSceneKey {
    fn from_scene(scene: &Scene) -> Self {
        Self {
            identity: scene.cache_identity(),
            revisions: [
                scene.structure_revision(),
                scene.representation_revision(),
                scene.volume_revision(),
                scene.segmentation_revision(),
                scene.mesh_revision(),
                scene.overlay_revision(),
                scene.interaction_revision(),
                scene.guide_revision(),
                scene.label_revision(),
                scene.property_revision(),
                scene.primitive_revision(),
                scene.generic_batch_revision(),
                scene.domain_visual_revision(),
                scene.ensemble_revision(),
                scene.interaction_state_revision(),
                scene.attribute_revision(),
            ],
        }
    }
}

impl<D: Device> Engine<D> {
    pub(super) fn update_temporal_scene_key(&mut self, scene: &Scene) -> bool {
        let key = TemporalSceneKey::from_scene(scene);
        self.temporal_scene_identity.replace(key) != Some(key)
    }
}

#[cfg(test)]
#[path = "temporal_scene_tests.rs"]
mod tests;
