//! Shared immutable assets supplied when restoring an authored scene.

use crate::{
    Error, HydrogenBondBinding, MolecularSurfaceBinding, SegmentationBinding, TrajectoryBinding,
    VolumeBinding,
};

/// Runtime assets kept outside the portable scene document.
/// Restoring these assets validates the same contracts as authoring a binding,
/// without recording edits or changing the document's semantic revision.
#[derive(Clone, Debug, Default)]
pub struct RuntimeBindings {
    /// Scalar grids addressed by their authored content identities.
    pub volumes: Vec<VolumeBinding>,
    /// Chemically classified donor, hydrogen and acceptor tables.
    pub hydrogen_bonds: Vec<HydrogenBondBinding>,
    /// Scientific surface results associated with molecular sources.
    pub molecular_surfaces: Vec<MolecularSurfaceBinding>,
    /// Categorical grids addressed by their authored content identities.
    pub segmentations: Vec<SegmentationBinding>,
    /// Shared resident coordinate pairs.
    pub trajectories: Vec<TrajectoryBinding>,
    /// Imperative resident intervals overriding authored trajectory playback.
    pub playback: Vec<super::PlaybackBinding>,
}

impl RuntimeBindings {
    pub(super) fn into_overlay(
        self,
        spec: &crate::SceneSpec,
        structures: &std::collections::BTreeMap<crate::StructureId, crate::source::MolecularSource>,
    ) -> Result<crate::overlay::OverlayBindings, Error> {
        let mut bindings = crate::overlay::OverlayBindings::default();
        for binding in self.volumes {
            bindings.insert(binding)?;
        }
        for binding in self.hydrogen_bonds {
            bindings.insert_hydrogen_bonds(binding)?;
        }
        for binding in self.molecular_surfaces {
            let molecule = structures.get(&binding.structure).ok_or(Error::MissingId)?;
            let mut matched = false;
            for recipe in spec.representations.values() {
                if recipe.structure_id() == Some(binding.structure)
                    && recipe
                        .molecular_surface_recipe()
                        .is_some_and(|(source, _, _)| {
                            source.content_hash == binding.source.content_hash
                        })
                {
                    binding.matches(recipe, molecule)?;
                    matched = true;
                }
            }
            if !matched {
                return Err(Error::InvalidSpec(
                    "scientific surface has no matching authored representation".to_owned(),
                ));
            }
            bindings.insert_molecular_surface(binding)?;
        }
        for binding in self.segmentations {
            bindings.insert_segmentation(binding)?;
        }
        for binding in self.trajectories {
            bindings.insert_trajectory(binding)?;
        }
        Ok(bindings)
    }
}
