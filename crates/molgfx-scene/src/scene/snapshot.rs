//! Snapshot restoration prepares a complete resolution before changing live state.

use super::{Scene, with_derived_bindings};
use crate::interop::SceneSnapshot;
use crate::{Error, PatchOperation, ScalarPropertyBinding, ScenePatch, SceneSpec, StructureId};
use std::collections::BTreeMap;

/// Shared assets no longer active after a restore, available to its inverse.
#[derive(Debug, Default)]
pub(super) struct SnapshotBindings {
    structures: BTreeMap<StructureId, BTreeMap<Box<str>, molgfx_core::MolecularSource>>,
    properties: BTreeMap<Box<str>, BTreeMap<Box<str>, ScalarPropertyBinding>>,
}

impl Scene {
    /// Captures complete authored state without copying molecular coordinates.
    #[must_use]
    pub fn snapshot(&self) -> SceneSnapshot {
        SceneSnapshot::capture(&self.spec)
    }

    /// Restores all authored state as one new revision.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid state, missing or mismatched local assets,
    /// exhausted identities or revisions, or failed lowering. No state changes
    /// until all bindings and the complete runtime resolution are prepared.
    pub fn restore_snapshot(&mut self, snapshot: &SceneSnapshot) -> Result<(), Error> {
        self.apply(&ScenePatch {
            base_revision: self.revision(),
            operations: vec![PatchOperation::RestoreSnapshot(Box::new(snapshot.clone()))],
        })
    }

    pub(super) fn apply_restoring(&mut self, patch: &ScenePatch) -> Result<(), Error> {
        // Even snapshots overwritten by a later operation must name valid assets.
        for operation in &patch.operations {
            if let PatchOperation::RestoreSnapshot(snapshot) = operation {
                snapshot.verify()?;
                let structures = self.snapshot_structures(&snapshot.scene)?;
                let _ = self.snapshot_properties(&snapshot.scene, &structures)?;
                self.validate_snapshot_overlays(&snapshot.scene)?;
            }
        }
        let spec = super::apply::candidate_spec(&self.spec, patch)?;
        let structures = self.snapshot_structures(&spec)?;
        let properties = self.snapshot_properties(&spec, &structures)?;
        self.validate_snapshot_overlays(&spec)?;
        let next_structure = self
            .next_structure
            .max(super::runtime::next_structure_id(&spec)?);
        let next_representation = self
            .next_representation
            .max(super::runtime::next_representation_id(&spec)?);
        let resolution = super::runtime::resolve_reusing(
            &spec,
            &structures,
            &properties,
            &self.overlay_bindings,
            &self.rows,
            Some(&self.structure_assets),
        )?;

        // Retain handles, not bulk data copies. Undo can reattach sources that
        // this restore removes, while the active binding maps remain exact.
        for (id, source) in std::mem::replace(&mut self.structures, structures) {
            if let Some(descriptor) = self.spec.structures.get(&id)
                && spec
                    .structures
                    .get(&id)
                    .is_none_or(|active| active.content_hash != descriptor.content_hash)
            {
                let _ = self
                    .snapshot_bindings
                    .structures
                    .entry(id)
                    .or_default()
                    .insert(descriptor.content_hash.clone(), source);
            }
        }
        for (name, binding) in std::mem::replace(&mut self.property_bindings, properties) {
            if spec.properties.get(&name) != Some(binding.spec()) {
                let _ = self
                    .snapshot_bindings
                    .properties
                    .entry(name)
                    .or_default()
                    .insert(binding.spec().source.content_hash.clone(), binding);
            }
        }
        self.spec = spec;
        self.next_structure = next_structure;
        self.next_representation = next_representation;
        self.install_resolution(resolution);
        Ok(())
    }

    fn snapshot_structures(
        &self,
        spec: &SceneSpec,
    ) -> Result<BTreeMap<StructureId, molgfx_core::MolecularSource>, Error> {
        let mut structures = BTreeMap::new();
        for (id, descriptor) in &spec.structures {
            let source = self.snapshot_source(*id, descriptor)?;
            let _ = structures.insert(*id, source.clone());
        }
        Ok(structures)
    }

    fn snapshot_source(
        &self,
        id: StructureId,
        descriptor: &crate::StructureSource,
    ) -> Result<&molgfx_core::MolecularSource, Error> {
        let source = self
            .snapshot_bindings
            .structures
            .get(&id)
            .and_then(|bindings| bindings.get(descriptor.content_hash.as_ref()))
            .or_else(|| self.structures.get(&id))
            .ok_or(Error::MissingSource { id })?;
        if descriptor.content_hash.is_empty()
            || descriptor.content_hash != super::hashing::structure_hash(source)
        {
            return Err(Error::InvalidSpec(format!(
                "snapshot source {} does not match its content hash",
                id.get(),
            )));
        }
        if let Some(matrix) = &descriptor.placement {
            crate::StructureSource::validate_placement(matrix)?;
        }
        Ok(source)
    }

    fn validate_snapshot_overlays(&self, spec: &SceneSpec) -> Result<(), Error> {
        for segmentation in spec.segmentations.values() {
            let binding = self
                .overlay_bindings
                .segmentation(&segmentation.source.content_hash)
                .ok_or_else(|| {
                    Error::InvalidSpec(format!(
                        "snapshot segmentation source '{}' is not bound",
                        segmentation.source.content_hash
                    ))
                })?;
            binding.matches(segmentation)?;
        }
        for volume in spec.volumes.values() {
            let binding = self
                .overlay_bindings
                .volume(&volume.source.content_hash)
                .ok_or_else(|| {
                    Error::InvalidSpec(format!(
                        "snapshot density source '{}' is not bound",
                        volume.source.content_hash,
                    ))
                })?;
            binding.matches(volume)?;
        }
        for trajectory in spec.trajectories.values() {
            let binding = self
                .overlay_bindings
                .trajectory(&trajectory.source.content_hash)
                .ok_or_else(|| {
                    Error::InvalidSpec(format!(
                        "snapshot trajectory source '{}' is not bound",
                        trajectory.source.content_hash,
                    ))
                })?;
            binding.matches(trajectory)?;
        }
        Ok(())
    }

    fn snapshot_properties(
        &self,
        spec: &SceneSpec,
        structures: &BTreeMap<StructureId, molgfx_core::MolecularSource>,
    ) -> Result<BTreeMap<Box<str>, ScalarPropertyBinding>, Error> {
        let mut bindings = BTreeMap::new();
        for (name, descriptor) in &spec.properties {
            let binding = self
                .property_bindings
                .get(name)
                .filter(|binding| binding.spec() == descriptor)
                .or_else(|| {
                    self.snapshot_bindings
                        .properties
                        .get(name)
                        .and_then(|bindings| bindings.get(descriptor.source.content_hash.as_ref()))
                        .filter(|binding| binding.spec() == descriptor)
                });
            if let Some(binding) = binding {
                let _ = bindings.insert(name.clone(), binding.clone());
            }
        }
        let bindings = with_derived_bindings(spec, structures, bindings)?;
        for (name, descriptor) in &spec.properties {
            let binding = bindings.get(name).ok_or_else(|| {
                Error::InvalidSpec(format!(
                    "snapshot property source '{}' is not bound to '{name}'",
                    descriptor.source.content_hash,
                ))
            })?;
            if binding.spec() != descriptor || binding.name() != name.as_ref() {
                return Err(Error::InvalidSpec(format!(
                    "snapshot property '{name}' does not match its descriptor"
                )));
            }
            let source = structures.get(&descriptor.structure).ok_or_else(|| {
                Error::InvalidSpec(format!(
                    "snapshot property '{name}' owns an unknown structure"
                ))
            })?;
            binding.validate(source.coordinates().len())?;
        }
        Ok(bindings)
    }
}
