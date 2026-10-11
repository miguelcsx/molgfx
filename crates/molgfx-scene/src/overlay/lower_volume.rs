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
    let field = binding.native()?;
    let volume = scene.add_volume(field.clone());
    let region = spec
        .region
        .map(|region| {
            molgfx_core::VolumeRegion::new(region.minimum, region.maximum, spec.dimensions)
        })
        .transpose()?;
    for presentation in &spec.presentations {
        if let super::VolumePresentation::FieldLines { options } = presentation {
            let hash = crate::spec::stable_json_hash(spec);
            let prefix = hash.get(..16).ok_or_else(|| {
                Error::InvalidSpec("field-line source hash is incomplete".to_owned())
            })?;
            let namespace = u64::from_str_radix(prefix, 16)
                .map_err(|_| Error::InvalidSpec("field-line source hash is invalid".to_owned()))?;
            super::field_lines::lower(scene, &field, *options, region, namespace)?;
            continue;
        }
        if let Some(lobes) = presentation.orbital_lobes()? {
            for mut config in lobes {
                if let Some(region) = region {
                    config = config.volume_region(region);
                }
                let _ = scene.represent(volume, config)?;
            }
            continue;
        }
        let mut config = presentation.native()?;
        if let Some(region) = region {
            // Regions belong to the grid descriptor, not individual styles.
            config = config.volume_region(region);
        }
        let _ = scene.represent(volume, config)?;
    }
    Ok(Some(volume))
}
