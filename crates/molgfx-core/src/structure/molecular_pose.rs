//! Immutable borrowed molecular samples. Capture and comparison cost O(1).

use crate::{MolecularSource, PlacedStructure, TrajectoryFrame, TrajectorySegment};

#[cfg(test)]
#[path = "molecular_pose_tests.rs"]
mod tests;

/// Retains the original molecule and the exact displayed coordinate sample.
/// Coordinate columns remain shared with their source or resident frame owners.
#[derive(Clone, Debug)]
pub struct MolecularPose {
    source: MolecularSource,
    trajectory: Option<TrajectorySegment>,
}

impl MolecularPose {
    /// Retains validated resident frames belonging to this topology without copying coordinates.
    ///
    /// # Errors
    /// Rejects a resident interval with a different atom count.
    pub fn new(
        source: MolecularSource,
        trajectory: Option<TrajectorySegment>,
    ) -> Result<Self, crate::CoreError> {
        if trajectory
            .as_ref()
            .is_some_and(|segment| segment.atom_count() != source.coordinates().len())
        {
            return Err(crate::CoreError::InvalidTrajectory {
                reason: "pose frames must match the molecular atom count",
            });
        }
        Ok(Self { source, trajectory })
    }

    /// Borrows the exact resident interval and sample used by a scientific calculation.
    #[must_use]
    pub const fn trajectory(&self) -> Option<&TrajectorySegment> {
        self.trajectory.as_ref()
    }

    /// Captures a placement without copying coordinates or derived tables.
    #[must_use]
    pub fn capture(placed: &PlacedStructure) -> Self {
        Self {
            source: placed.source.clone(),
            trajectory: placed.trajectory().cloned(),
        }
    }

    /// Original immutable molecular storage and topology.
    #[must_use]
    pub const fn source(&self) -> &MolecularSource {
        &self.source
    }

    /// Borrows the static coordinates or the first resident frame.
    #[must_use]
    pub fn positions(&self) -> &[[f32; 3]] {
        match &self.trajectory {
            Some(segment) => segment.start().positions(),
            None => self.source.coordinates(),
        }
    }

    /// Borrows the second resident frame when this is a trajectory sample.
    #[must_use]
    pub fn end_positions(&self) -> Option<&[[f32; 3]]> {
        self.trajectory
            .as_ref()
            .map(|segment| segment.end().positions())
    }

    /// Exact Cartesian interpolation weight for the two resident frames.
    #[must_use]
    pub fn interpolation(&self) -> Option<f64> {
        self.trajectory
            .as_ref()
            .map(TrajectorySegment::interpolation)
    }

    /// Tests the exact source, retained frame storage and sample time in O(1).
    #[must_use]
    pub fn matches(&self, placed: &PlacedStructure) -> bool {
        self.matches_interval(placed)
            && match (self.trajectory.as_ref(), placed.trajectory()) {
                (Some(a), Some(b)) => a.sample_seconds().to_bits() == b.sample_seconds().to_bits(),
                (None, None) => true,
                _ => false,
            }
    }

    /// Tests the retained coordinate interval in O(1), independent of its sample time.
    /// Conservative motion candidates remain valid throughout this exact interval.
    #[must_use]
    pub fn matches_interval(&self, placed: &PlacedStructure) -> bool {
        self.source.identity() == placed.source.identity()
            && self.source.coordinate_revision() == placed.source.coordinate_revision()
            && same_interval(self.trajectory.as_ref(), placed.trajectory())
    }

    /// Compares the molecular source and retained interval in O(1), ignoring sample time.
    #[must_use]
    pub fn shares_interval(&self, other: &Self) -> bool {
        self.source.identity() == other.source.identity()
            && self.source.coordinate_revision() == other.source.coordinate_revision()
            && same_interval(self.trajectory.as_ref(), other.trajectory.as_ref())
    }
}

impl PartialEq for MolecularPose {
    fn eq(&self, other: &Self) -> bool {
        self.source.identity() == other.source.identity()
            && self.source.coordinate_revision() == other.source.coordinate_revision()
            && same_sample(self.trajectory.as_ref(), other.trajectory.as_ref())
    }
}

fn same_sample(a: Option<&TrajectorySegment>, b: Option<&TrajectorySegment>) -> bool {
    same_interval(a, b)
        && match (a, b) {
            (Some(a), Some(b)) => a.sample_seconds().to_bits() == b.sample_seconds().to_bits(),
            (None, None) => true,
            _ => false,
        }
}

fn same_frame(a: Option<&TrajectoryFrame>, b: Option<&TrajectoryFrame>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => same_resident(a, b),
        _ => false,
    }
}

fn same_resident(a: &TrajectoryFrame, b: &TrajectoryFrame) -> bool {
    a.index() == b.index()
        && a.time_seconds().to_bits() == b.time_seconds().to_bits()
        && std::ptr::eq(a.positions(), b.positions())
}

fn same_interval(a: Option<&TrajectorySegment>, b: Option<&TrajectorySegment>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => {
            a.interpolation_kind() == b.interpolation_kind()
                && same_frame(a.before(), b.before())
                && same_frame(a.after(), b.after())
                && [a.start(), a.end()]
                    .into_iter()
                    .zip([b.start(), b.end()])
                    .all(|(a, b)| same_resident(a, b))
        }
        _ => false,
    }
}
