//! Static scientific facts retain source owners and placement without copying coordinates.
use super::InteractionEdge;
use crate::{CoreError, MolecularPose, Scene, StructureHandle};
use molgfx_math::Mat4;

#[derive(Clone, PartialEq, Debug)]
pub(super) struct StaticCoordinates {
    structure: StructureHandle,
    pose: MolecularPose,
    model_to_world: Mat4,
}

impl InteractionEdge {
    /// Restricts a static scientific fact to its original molecular inputs and placements.
    /// World-space guides can leave both dependencies absent. Comparison costs O(1).
    /// # Errors
    /// Refuses absent molecular placements.
    pub fn with_static_coordinates(
        self,
        structures: [Option<StructureHandle>; 2],
        scene: &Scene,
    ) -> Result<Self, CoreError> {
        let mut inputs = [None, None];
        for (input, structure) in inputs.iter_mut().zip(structures) {
            if let Some(structure) = structure {
                let placed = scene.structure(structure).ok_or(CoreError::StaleHandle)?;
                *input = Some((structure, placed.model_to_world));
            }
        }
        self.with_static_coordinates_at(inputs, scene)
    }

    /// Original immutable placements supporting this scientific fact.
    #[must_use]
    pub fn static_coordinates(&self) -> [Option<(StructureHandle, Mat4)>; 2] {
        self.static_coordinates.each_ref().map(|input| {
            input
                .as_ref()
                .map(|input| (input.structure, input.model_to_world))
        })
    }

    /// Whether every original molecular input is currently displayed unchanged.
    /// Entering playback hides static facts; clearing it restores their original status.
    #[must_use]
    pub fn coordinates_match(&self, scene: &Scene) -> bool {
        self.static_coordinates.iter().flatten().all(|input| {
            scene.structure(input.structure).is_some_and(|placed| {
                input.pose.matches(placed)
                    && input.model_to_world.to_cols_array().map(f32::to_bits)
                        == placed.model_to_world.to_cols_array().map(f32::to_bits)
            })
        })
    }

    pub(crate) fn validate_coordinate_sources(&self, scene: &Scene) -> Result<(), CoreError> {
        for input in self.static_coordinates.iter().flatten() {
            let placed = scene
                .structure(input.structure)
                .ok_or(CoreError::StaleHandle)?;
            if input.pose.source().identity() != placed.source.identity()
                || input.pose.source().coordinate_revision() != placed.source.coordinate_revision()
            {
                return Err(CoreError::InvalidInteraction {
                    reason: "interaction coordinates belong to another molecular source",
                });
            }
        }
        Ok(())
    }

    pub(crate) fn with_static_coordinates_at(
        mut self,
        inputs: [Option<(StructureHandle, Mat4)>; 2],
        scene: &Scene,
    ) -> Result<Self, CoreError> {
        self.static_coordinates = [None, None];
        for (target, input) in self.static_coordinates.iter_mut().zip(inputs) {
            if let Some((structure, model_to_world)) = input {
                let placed = scene.structure(structure).ok_or(CoreError::StaleHandle)?;
                if !model_to_world.is_finite() {
                    return Err(CoreError::InvalidInteraction {
                        reason: "interaction input placement must be finite",
                    });
                }
                *target = Some(StaticCoordinates {
                    structure,
                    pose: MolecularPose::new(placed.source.clone(), None)?,
                    model_to_world,
                });
            }
        }
        Ok(self)
    }
}
