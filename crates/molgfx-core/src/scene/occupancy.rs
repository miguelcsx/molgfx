//! GPU-resident temporal occupancy volume declarations.

use super::{BoundOccupancy, Scene, StoredVolume};
use crate::{AtomSelection, CoreError, OccupancyStream, StructureHandle, VolumeHandle};
use std::sync::Arc;

impl Scene {
    /// Binds a model-space occupancy grid to selected atoms of one structure.
    ///
    /// The evolving scalar field lives only on the GPU. This call stores one
    /// sorted atom-row list and no host voxel allocation.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::StaleHandle`] for a removed structure.
    pub fn add_occupancy_stream(
        &mut self,
        structure: StructureHandle,
        selection: &AtomSelection,
        stream: OccupancyStream,
    ) -> Result<VolumeHandle, CoreError> {
        let rows = selected_rows(self, structure, selection)?;
        self.volume_revision = self.volume_revision.wrapping_add(1);
        Ok(VolumeHandle(self.volumes.insert(StoredVolume {
            value: None,
            occupancy: Some(BoundOccupancy {
                stream,
                structure,
                atom_rows: Arc::from(rows),
            }),
            revision: 0,
        })))
    }

    /// Replaces a temporal occupancy declaration while preserving its handle.
    ///
    /// The renderer discards the prior GPU history and starts the new grid at
    /// zero. No host voxel grid is created.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::StaleHandle`] for an absent structure, volume or a
    /// handle that refers to a static scalar grid.
    pub fn replace_occupancy_stream(
        &mut self,
        handle: VolumeHandle,
        structure: StructureHandle,
        selection: &AtomSelection,
        stream: OccupancyStream,
    ) -> Result<(), CoreError> {
        let rows = selected_rows(self, structure, selection)?;
        let stored = self
            .volumes
            .get_mut(handle.0)
            .filter(|stored| stored.occupancy.is_some())
            .ok_or(CoreError::StaleHandle)?;
        stored.value = None;
        stored.occupancy = Some(BoundOccupancy {
            stream,
            structure,
            atom_rows: Arc::from(rows),
        });
        stored.revision = stored.revision.wrapping_add(1);
        self.volume_revision = self.volume_revision.wrapping_add(1);
        Ok(())
    }

    /// Resolves a dynamic occupancy declaration and its selected source rows.
    #[must_use]
    pub fn occupancy_stream(
        &self,
        handle: VolumeHandle,
    ) -> Option<(&OccupancyStream, StructureHandle, &[u32])> {
        let value = self.volumes.get(handle.0)?.occupancy.as_ref()?;
        Some((&value.stream, value.structure, &value.atom_rows))
    }

    /// Whether this scene requests temporal occupancy accumulation.
    #[must_use]
    pub fn has_occupancy_stream(&self) -> bool {
        self.volumes
            .iter()
            .any(|(_, volume)| volume.occupancy.is_some())
    }

    /// Removes a dynamic occupancy stream and invalidates its volume handle.
    pub fn remove_occupancy_stream(&mut self, handle: VolumeHandle) -> bool {
        let Some(stored) = self.volumes.get(handle.0) else {
            return false;
        };
        if stored.occupancy.is_none() {
            return false;
        }
        let removed = self.volumes.remove(handle.0).is_some();
        if removed {
            self.volume_revision = self.volume_revision.wrapping_add(1);
        }
        removed
    }
}

fn selected_rows(
    scene: &Scene,
    structure: StructureHandle,
    selection: &AtomSelection,
) -> Result<Vec<u32>, CoreError> {
    let atom_count = scene
        .structure(structure)
        .ok_or(CoreError::StaleHandle)?
        .atoms
        .len();
    let mut rows = Vec::new();
    selection.for_each(atom_count, |row| rows.push(row));
    rows.sort_unstable();
    rows.dedup();
    Ok(rows)
}
