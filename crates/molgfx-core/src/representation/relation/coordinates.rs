//! O(1) validity gates over retained molecular samples and motion intervals.

use super::{RelationBatch, SpatialAnchor, invalid};
use crate::{CoreError, MolecularPose, RowDomain, Scene, StructureHandle};

#[cfg(test)]
#[path = "coordinates_tests.rs"]
mod tests;

/// Original molecular input supporting one immutable scientific relation table.
#[derive(Clone, PartialEq, Debug)]
pub struct RelationCoordinates {
    structure: StructureHandle,
    pose: MolecularPose,
    interval: bool,
}

impl RelationCoordinates {
    /// Molecular placement addressed by the relation rows.
    #[must_use]
    pub const fn structure(&self) -> StructureHandle {
        self.structure
    }

    /// Retained original calculation input without copied atom coordinates.
    #[must_use]
    pub const fn pose(&self) -> &MolecularPose {
        &self.pose
    }

    /// Whether validity covers every sample within the exact retained interval.
    #[must_use]
    pub const fn covers_interval(&self) -> bool {
        self.interval
    }

    /// Checks source ownership before insertion, permitting a retained stale result.
    ///
    /// # Errors
    /// Rejects missing placements and scientific inputs from another molecule.
    pub fn validate_source(&self, scene: &Scene) -> Result<(), CoreError> {
        let placed = scene
            .structure(self.structure)
            .ok_or(CoreError::StaleHandle)?;
        let source = self.pose.source();
        if source.identity() != placed.source.identity()
            || source.coordinate_revision() != placed.source.coordinate_revision()
        {
            return Err(invalid(
                "relation coordinates belong to another molecular source",
            ));
        }
        Ok(())
    }

    /// Determines table validity in O(1), including exact restoration after reversal.
    #[must_use]
    pub fn matches(&self, scene: &Scene) -> bool {
        scene.structure(self.structure).is_some_and(|placed| {
            if self.interval {
                self.pose.matches_interval(placed)
            } else {
                self.pose.matches(placed)
            }
        })
    }
}

impl RelationBatch {
    /// Pins classified snapshot rows to their exact original coordinate sample.
    ///
    /// # Errors
    /// All endpoints must address atoms of one molecular placement.
    pub fn with_molecular_sample(self, pose: MolecularPose) -> Result<Self, CoreError> {
        self.with_coordinates(pose, false)
    }

    /// Pins conservative candidates to their complete original coordinate interval.
    ///
    /// # Errors
    /// All endpoints must address atoms of one molecular placement.
    pub fn with_molecular_interval(self, pose: MolecularPose) -> Result<Self, CoreError> {
        self.with_coordinates(pose, true)
    }

    /// Original input controlling this scientific table's visibility.
    #[must_use]
    pub const fn molecular_coordinates(&self) -> Option<&RelationCoordinates> {
        self.coordinates.as_ref()
    }

    fn with_coordinates(mut self, pose: MolecularPose, interval: bool) -> Result<Self, CoreError> {
        let Some(first) = self.relations.first() else {
            return Err(invalid("molecular relations require atom endpoints"));
        };
        let SpatialAnchor::Entity(first) = first.start else {
            return Err(invalid("molecular relations require atom endpoints"));
        };
        let RowDomain::Atoms(structure) = first.domain() else {
            return Err(invalid("molecular relations require atom endpoints"));
        };
        if self.relations.iter().any(|relation| {
            [relation.start, relation.end].into_iter().any(|anchor| {
                !matches!(anchor, SpatialAnchor::Entity(entity)
                    if entity.domain() == RowDomain::Atoms(structure))
            })
        }) {
            return Err(invalid("molecular relations must address one atom source"));
        }
        self.coordinates = Some(RelationCoordinates {
            structure,
            pose,
            interval,
        });
        Ok(self)
    }
}
