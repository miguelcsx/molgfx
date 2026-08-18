//! Scene lifecycle for caller-owned categorical label volumes.

use super::{Scene, StoredRepresentation, StoredSegmentation};
use crate::handle::{RepresentationHandle, SegmentationHandle};
use crate::representation::{Representation, RepresentationKind, RepresentationTarget};
use crate::{SegmentStyleTable, SegmentedVolume};

#[cfg(test)]
#[path = "segmentation_tests.rs"]
mod tests;

impl Scene {
    /// Stores an immutable shared categorical label grid.
    pub fn add_segmented_volume(&mut self, volume: SegmentedVolume) -> SegmentationHandle {
        self.segmentation_revision = self.segmentation_revision.wrapping_add(1);
        SegmentationHandle(self.segmentations.insert(StoredSegmentation {
            value: volume,
            revision: 0,
        }))
    }

    /// Resolves a categorical label-grid handle.
    #[must_use]
    pub fn segmented_volume(&self, handle: SegmentationHandle) -> Option<&SegmentedVolume> {
        self.segmentations.get(handle.0).map(|stored| &stored.value)
    }

    /// Replaces a categorical grid while preserving its stable handle.
    ///
    /// # Errors
    ///
    /// Returns [`crate::CoreError::StaleHandle`] when the grid was removed.
    pub fn replace_segmented_volume(
        &mut self,
        handle: SegmentationHandle,
        volume: SegmentedVolume,
    ) -> Result<(), crate::CoreError> {
        let stored = self
            .segmentations
            .get_mut(handle.0)
            .ok_or(crate::CoreError::StaleHandle)?;
        stored.value = volume;
        stored.revision = stored.revision.wrapping_add(1);
        self.segmentation_revision = self.segmentation_revision.wrapping_add(1);
        Ok(())
    }

    /// Removes a categorical grid and invalidates its handle.
    pub fn remove_segmented_volume(
        &mut self,
        handle: SegmentationHandle,
    ) -> Option<SegmentedVolume> {
        let removed = self
            .segmentations
            .remove(handle.0)
            .map(|stored| stored.value);
        if removed.is_some() {
            self.segmentation_revision = self.segmentation_revision.wrapping_add(1);
        }
        removed
    }

    /// Adds one independently styled representation of a categorical grid.
    ///
    /// # Errors
    ///
    /// Returns [`crate::CoreError::StaleHandle`] when the grid was removed.
    pub fn represent_segmented_volume(
        &mut self,
        volume: SegmentationHandle,
        styles: SegmentStyleTable,
    ) -> Result<RepresentationHandle, crate::CoreError> {
        if self.segmentations.get(volume.0).is_none() {
            return Err(crate::CoreError::StaleHandle);
        }
        let mut representation = Representation::new(
            RepresentationTarget::SegmentedVolume(volume),
            RepresentationKind::Segmentation,
        );
        representation.segmentation.styles = styles;
        self.representation_revision = self.representation_revision.wrapping_add(1);
        Ok(RepresentationHandle(self.representations.insert(
            StoredRepresentation {
                value: representation,
                revision: 0,
            },
        )))
    }

    /// Counter keying categorical-grid texture reconciliation.
    #[must_use]
    pub const fn segmentation_revision(&self) -> u64 {
        self.segmentation_revision
    }

    /// Revision of one categorical grid's contents.
    #[must_use]
    pub fn segmentation_content_revision(&self, handle: SegmentationHandle) -> Option<u64> {
        self.segmentations
            .get(handle.0)
            .map(|stored| stored.revision)
    }
}
