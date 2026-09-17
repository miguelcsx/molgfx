//! Scene lifecycle for caller-decoded dynamic covalent topology.

use crate::{BondTopologySegment, CoreError, Scene, StructureHandle};

#[cfg(test)]
#[path = "topology_tests.rs"]
mod tests;

impl Scene {
    /// Replaces the two resident connectivity frames for one fixed atom table.
    ///
    /// # Errors
    ///
    /// Returns a typed error for a stale structure or mismatched atom count.
    pub fn set_bond_topology_segment(
        &mut self,
        handle: StructureHandle,
        segment: BondTopologySegment,
    ) -> Result<(), CoreError> {
        let placed = self.structure_mut(handle).ok_or(CoreError::StaleHandle)?;
        if segment.start().atom_count() != placed.atoms.len() {
            return Err(CoreError::InvalidTrajectory {
                reason: "dynamic topology atom count must match the placed structure",
            });
        }
        placed.replace_bond_topology(segment);
        Ok(())
    }

    /// Advances connectivity birth/death weights without allocating.
    ///
    /// # Errors
    ///
    /// Returns a typed error for a stale structure, absent interval or time
    /// outside its closed resident interval.
    pub fn set_bond_topology_time(
        &mut self,
        handle: StructureHandle,
        sample_seconds: f32,
    ) -> Result<(), CoreError> {
        self.structure_mut(handle)
            .ok_or(CoreError::StaleHandle)?
            .set_bond_topology_time(sample_seconds)
    }

    /// Removes dynamic connectivity and restores the immutable source bonds.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::StaleHandle`] after structure removal.
    pub fn clear_bond_topology(&mut self, handle: StructureHandle) -> Result<bool, CoreError> {
        Ok(self
            .structure_mut(handle)
            .ok_or(CoreError::StaleHandle)?
            .clear_bond_topology())
    }
}
