//! Chemical tables and their descriptors cross one atomic scientific-input boundary.

use crate::{Error, HydrogenBondBinding, InteractionSpec, Scene, ScenePatch, SceneSpec};

impl Scene {
    /// Installs classified hydrogen-bond input and its authored changes in one revision.
    ///
    /// The result retains its calculation pose; candidates may be published at any
    /// time inside the same resident interval. Failed validation preserves the old
    /// scene and leaves the content identity available for a corrected retry.
    ///
    /// # Errors
    /// Refuses a missing descriptor, stale input, conflicting identity or invalid patch.
    pub fn apply_with_hydrogen_bonds(
        &mut self,
        patch: &ScenePatch,
        binding: HydrogenBondBinding,
    ) -> Result<(), Error> {
        let candidate = super::apply::candidate_spec(&self.spec, patch)?;
        let binding = self.validate_hydrogen_bond_input(binding, &candidate)?;
        let mut bindings = self.overlay_bindings.clone();
        bindings.insert_hydrogen_bonds(binding)?;
        self.apply_with_overlay_bindings(patch, Some(bindings))
    }

    pub(super) fn validate_hydrogen_bond_input(
        &self,
        mut binding: HydrogenBondBinding,
        spec: &SceneSpec,
    ) -> Result<HydrogenBondBinding, Error> {
        let mut matched = false;
        for interaction in spec.interactions.values() {
            let InteractionSpec::HydrogenBonds(spec) = interaction else {
                continue;
            };
            if spec.source.content_hash != binding.source().content_hash {
                continue;
            }
            matched = true;
            if binding.pose().is_none() {
                binding = binding.at_pose(self.molecular_pose(spec.structure)?);
            }
            let placed = self
                .structure_handle(spec.structure)
                .and_then(|handle| self.resolved().structure(handle))
                .ok_or(Error::MissingId)?;
            let valid = binding.pose().is_some_and(|pose| {
                if binding.candidate_distance().is_some() {
                    pose.matches_interval(placed)
                } else {
                    pose.matches(placed)
                }
            });
            if !valid {
                return Err(Error::InvalidSpec(
                    "hydrogen-bond results belong to a different molecular coordinate input"
                        .to_owned(),
                ));
            }
        }
        if !matched {
            return Err(Error::InvalidSpec(
                "hydrogen-bond input requires a matching authored descriptor".to_owned(),
            ));
        }
        Ok(binding)
    }
}
