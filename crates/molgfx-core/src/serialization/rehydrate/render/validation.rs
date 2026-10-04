//! Validate restored presentation controls against their bound source grids.

use super::{Scene, invalid};
use crate::{CoreError, Representation, RepresentationTarget};

pub(super) fn validate_regions(
    scene: &Scene,
    target: RepresentationTarget,
    value: &Representation,
) -> Result<(), CoreError> {
    if let RepresentationTarget::Volume(handle) = target {
        let (dimensions, transform) = scene
            .volumes
            .get(handle.0)
            .and_then(|volume| volume.grid(scene))
            .ok_or(CoreError::StaleHandle)?;
        value.volume.validate_grid(dimensions, transform)?;
    } else if value.volume.region.is_some() {
        return invalid("volume region is attached to a non-volume target");
    }
    if let RepresentationTarget::SegmentedVolume(handle) = target {
        let grid = scene
            .segmented_volume(handle)
            .ok_or(CoreError::StaleHandle)?;
        value.segmentation.validate_grid(grid)?;
    } else if value.segmentation.region.is_some() {
        return invalid("segmentation region is attached to a non-segmentation target");
    }
    Ok(())
}
