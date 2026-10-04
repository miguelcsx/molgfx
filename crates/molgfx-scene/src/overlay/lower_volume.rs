//! Resolving independently styled presentations over one shared scalar grid.

use super::{OverlayBindings, VolumeSpec};
use crate::Error;
use molgfx_core::{Scene, VolumeHandle};

pub(super) fn lower_volume(
    scene: &mut Scene,
    spec: &VolumeSpec,
    bindings: &OverlayBindings,
) -> Result<Option<VolumeHandle>, Error> {
    let Some(binding) = bindings.volume(&spec.source.content_hash) else {
        return Ok(None);
    };
    binding.matches(spec)?;
    let volume = scene.add_volume(binding.native()?);
    let region = spec
        .region
        .map(|region| {
            molgfx_core::VolumeRegion::new(region.minimum, region.maximum, spec.dimensions)
        })
        .transpose()?;
    for presentation in &spec.presentations {
        let mut config = presentation.native()?;
        if let Some(region) = region {
            // Regions belong to the grid descriptor, not individual styles.
            config = config.volume_region(region);
        }
        let _ = scene.represent(volume, config)?;
    }
    Ok(Some(volume))
}
