//! Scene editing for bounded, caller-supplied trajectory presentation.

use crate::{CoreError, Scene, StructureHandle, TrajectorySegment};

#[cfg(test)]
#[path = "trajectory_tests.rs"]
mod tests;

impl Scene {
    /// Replaces the two resident frames for one topology-stable structure.
    ///
    /// The frame arrays remain shared with the caller. A conservative BVH is
    /// rebuilt once for the pair and remains valid for every linear sample.
    ///
    /// # Errors
    ///
    /// Returns a typed error for a stale handle or mismatched atom count.
    pub fn set_trajectory_segment(
        &mut self,
        handle: StructureHandle,
        segment: TrajectorySegment,
    ) -> Result<(), CoreError> {
        let placed = self.structure_mut(handle).ok_or(CoreError::StaleHandle)?;
        if segment.atom_count() != placed.atoms.len() as usize {
            return Err(CoreError::InvalidTrajectory {
                reason: "trajectory atom count must match the placed topology",
            });
        }
        placed.replace_trajectory(segment);
        Ok(())
    }

    /// Advances presentation time inside the resident frame interval.
    ///
    /// # Errors
    ///
    /// Returns a typed error for a stale handle, absent interval or time
    /// outside the closed resident interval.
    pub fn set_trajectory_time(
        &mut self,
        handle: StructureHandle,
        sample_seconds: f32,
    ) -> Result<(), CoreError> {
        self.structure_mut(handle)
            .ok_or(CoreError::StaleHandle)?
            .set_trajectory_time(sample_seconds)
    }

    /// Removes the active interval and restores parsed coordinates.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::StaleHandle`] after structure removal.
    pub fn clear_trajectory(&mut self, handle: StructureHandle) -> Result<bool, CoreError> {
        Ok(self
            .structure_mut(handle)
            .ok_or(CoreError::StaleHandle)?
            .clear_trajectory())
    }
}
