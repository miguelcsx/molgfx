//! Mutable semantic scene and atomic transaction boundary.

use crate::SceneTransaction;
use crate::error::{Error, PatchError};
use crate::id::{RepresentationId, StructureId};
use crate::representation::{SceneItem, Selection};
use crate::scene_runtime::{
    apply_runtime_edits, candidate_spec, canonical_selection_count, next_representation_id,
    next_structure_id, resolve, structure_hash,
};
use crate::spec::{InteractionChannel, PatchOperation, ScenePatch, SceneSpec, StructureSource};
use molgfx_core::{RepresentationHandle, SelectionHandle};
use std::collections::BTreeMap;

/// Mutable scene state backed by one resolved renderer scene.
#[derive(Debug)]
pub struct Scene {
    spec: SceneSpec,
    resolved: molgfx_core::Scene,
    structures: BTreeMap<StructureId, molframe::Structure>,
    representations: BTreeMap<RepresentationId, RepresentationHandle>,
    selections: BTreeMap<String, SelectionHandle>,
    next_structure: u64,
    next_representation: u64,
}

pub(super) struct Resolution {
    pub(super) scene: molgfx_core::Scene,
    pub(super) representations: BTreeMap<RepresentationId, RepresentationHandle>,
    pub(super) selections: BTreeMap<String, SelectionHandle>,
}

impl Scene {
    /// Builds a scene over a shared immutable `MolFrame` snapshot.
    ///
    /// # Errors
    ///
    /// Returns a validation error if the molecular source cannot be adapted.
    pub fn from_structure(structure: &molframe::Structure) -> Result<Self, Error> {
        let structure_id = StructureId(1);
        let mut spec = SceneSpec::empty();
        let _ = spec.structures.insert(
            structure_id,
            StructureSource {
                content_hash: structure_hash(structure),
                uri: None,
                format: None,
            },
        );
        let mut structures = BTreeMap::new();
        let _ = structures.insert(structure_id, structure.clone());
        Ok(Self {
            spec,
            resolved: molgfx_core::Scene::from_structure(structure)?,
            structures,
            representations: BTreeMap::new(),
            selections: BTreeMap::new(),
            next_structure: 2,
            next_representation: 1,
        })
    }

    /// Resolves a portable specification against shared local molecular bindings.
    ///
    /// Coordinates remain owned by the supplied `MolFrame` handles and are not
    /// present in the specification. Every declared structure must have exactly
    /// one binding with the same semantic ID and content hash.
    ///
    /// # Errors
    ///
    /// Returns an error for missing, extra, mismatched, or invalid bindings.
    pub fn from_spec(
        spec: SceneSpec,
        structures: BTreeMap<StructureId, molframe::Structure>,
    ) -> Result<Self, Error> {
        if spec.version != 1 {
            return Err(Error::InvalidSpec(format!(
                "unsupported SceneSpec version {}",
                spec.version
            )));
        }
        spec.validate_camera()?;
        if spec.structures.len() != structures.len()
            || spec
                .structures
                .keys()
                .any(|identity| !structures.contains_key(identity))
        {
            return Err(Error::InvalidSpec(
                "structure bindings must exactly match SceneSpec identities".to_owned(),
            ));
        }
        for (identity, structure) in &structures {
            let source = spec.structures.get(identity).ok_or_else(|| {
                Error::InvalidSpec("structure binding has no source descriptor".to_owned())
            })?;
            if !source.content_hash.is_empty()
                && source.content_hash.as_ref() != structure_hash(structure).as_ref()
            {
                return Err(Error::InvalidSpec(format!(
                    "structure {} does not match its content hash",
                    identity.get()
                )));
            }
        }
        spec.validate_selections()?;
        let resolution = resolve(&spec, &structures)?;
        let next_structure = next_structure_id(&spec)?;
        let next_representation = next_representation_id(&spec)?;
        Ok(Self {
            spec,
            resolved: resolution.scene,
            structures,
            representations: resolution.representations,
            selections: resolution.selections,
            next_structure,
            next_representation,
        })
    }

    /// Adds one immutable representation and returns its semantic identity.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid selection or representation value.
    pub fn add<T: SceneItem>(&mut self, item: T) -> Result<RepresentationId, Error> {
        let mut representation = item.into_spec();
        if representation.structure.is_none() {
            if self.structures.len() != 1 {
                return Err(Error::InvalidSpec(
                    "multi-structure scenes require an explicit representation structure"
                        .to_owned(),
                ));
            }
            representation.structure = self.structures.first_key_value().map(|(id, _)| *id);
        }
        representation.validate()?;
        let native = representation.native()?;
        let structure = representation.structure.ok_or_else(|| {
            Error::InvalidSpec("representation has no structure target".to_owned())
        })?;
        if !self.structures.contains_key(&structure) {
            return Err(Error::InvalidSpec(
                "representation targets an unknown structure".to_owned(),
            ));
        }
        let selection_key = format!(
            "{}:{}",
            structure.get(),
            representation.target.stable_hash()?
        );
        let selection = if let Some(selection) = self.selections.get(&selection_key).copied() {
            selection
        } else {
            let core_structure = self
                .structures
                .keys()
                .position(|id| *id == structure)
                .and_then(|index| {
                    self.resolved
                        .structures()
                        .nth(index)
                        .map(|(handle, _)| handle)
                })
                .ok_or_else(|| Error::InvalidSpec("structure binding is unresolved".to_owned()))?;
            let all_structures = self.resolved.select_str(representation.selection())?;
            let rows = self
                .resolved
                .selection_for(all_structures, core_structure)
                .cloned()
                .ok_or_else(|| Error::InvalidSpec("selection did not resolve".to_owned()))?;
            let selection = self
                .resolved
                .add_structure_selection(core_structure, rows)?;
            let _ = self.selections.insert(selection_key, selection);
            selection
        };
        let handle = self.resolved.represent(selection, native)?;
        if !representation.visible {
            self.resolved.hide(handle);
        }
        let id = RepresentationId(self.next_representation);
        self.next_representation = self.next_representation.checked_add(1).ok_or_else(|| {
            Error::InvalidSpec("representation identity space is exhausted".to_owned())
        })?;
        let _ = self.representations.insert(id, handle);
        let _ = self.spec.representations.insert(id, representation);
        self.spec.revision = self.spec.revision.wrapping_add(1);
        self.spec.revisions.selection = self.spec.revisions.selection.wrapping_add(1);
        self.spec.revisions.appearance = self.spec.revisions.appearance.wrapping_add(1);
        Ok(id)
    }

    /// Adds another shared `MolFrame` snapshot without copying coordinates.
    ///
    /// # Errors
    ///
    /// Returns an error if the source cannot be adapted or IDs are exhausted.
    pub fn add_structure(&mut self, structure: &molframe::Structure) -> Result<StructureId, Error> {
        let id = StructureId(self.next_structure);
        let next_structure = self.next_structure.checked_add(1).ok_or_else(|| {
            Error::InvalidSpec("structure identity space is exhausted".to_owned())
        })?;
        let mut candidate_structures = self.structures.clone();
        let _ = candidate_structures.insert(id, structure.clone());
        let mut candidate = self.spec.clone();
        let _ = candidate.structures.insert(
            id,
            StructureSource {
                content_hash: structure_hash(structure),
                uri: None,
                format: None,
            },
        );
        candidate.revision = candidate.revision.wrapping_add(1);
        candidate.revisions.topology = candidate.revisions.topology.wrapping_add(1);
        candidate.revisions.coordinates = candidate.revisions.coordinates.wrapping_add(1);
        let resolution = resolve(&candidate, &candidate_structures)?;
        self.next_structure = next_structure;
        self.structures = candidate_structures;
        self.spec = candidate;
        self.resolved = resolution.scene;
        self.representations = resolution.representations;
        self.selections = resolution.selections;
        Ok(id)
    }

    /// Current canonical immutable scene state.
    #[must_use]
    pub const fn spec(&self) -> &SceneSpec {
        &self.spec
    }

    /// Clones the small semantic description without copying molecular data.
    #[must_use]
    pub fn to_spec(&self) -> SceneSpec {
        self.spec.clone()
    }

    /// Current semantic revision.
    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.spec.revision
    }

    /// Shows or hides one representation through an atomic one-operation patch.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown ID or failed runtime update.
    pub fn set_visible(&mut self, id: RepresentationId, visible: bool) -> Result<(), Error> {
        self.apply(&ScenePatch {
            base_revision: self.revision(),
            operations: vec![PatchOperation::SetVisibility { id, visible }],
        })
    }

    /// Sets representation opacity without rebuilding molecular records.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown ID or opacity outside zero to one.
    pub fn set_opacity(&mut self, id: RepresentationId, opacity: f32) -> Result<(), Error> {
        self.apply(&ScenePatch {
            base_revision: self.revision(),
            operations: vec![PatchOperation::SetOpacity { id, opacity }],
        })
    }

    /// Replaces a representation's immutable visual program without rebuilding geometry.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown ID or invalid expression graph.
    pub fn set_visual(
        &mut self,
        id: RepresentationId,
        visual: Option<crate::VisualStyle>,
    ) -> Result<(), Error> {
        self.apply(&ScenePatch {
            base_revision: self.revision(),
            operations: vec![PatchOperation::SetVisual { id, visual }],
        })
    }

    /// Updates one typed visual parameter without recompiling its program.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown representation, parameter, or invalid value.
    pub fn set_parameter<T: crate::ParameterType>(
        &mut self,
        id: RepresentationId,
        parameter: &crate::Parameter<T>,
        value: T,
    ) -> Result<(), Error> {
        self.apply(&ScenePatch {
            base_revision: self.revision(),
            operations: vec![PatchOperation::SetParameter {
                id,
                name: parameter.name().into(),
                value: Some(value.into_parameter_value()),
            }],
        })
    }

    /// Sets the scene focus query.
    ///
    /// # Errors
    ///
    /// Returns an error if the query or runtime update is invalid.
    pub fn focus(&mut self, selection: impl Into<Selection>) -> Result<(), Error> {
        self.apply(&ScenePatch {
            base_revision: self.revision(),
            operations: vec![PatchOperation::SetFocus {
                selection: Some(selection.into()),
            }],
        })
    }

    /// Replaces one semantic interaction channel without rebuilding geometry.
    ///
    /// # Errors
    ///
    /// Returns an error if the update would make the scene invalid.
    pub fn set_interaction(
        &mut self,
        channel: InteractionChannel,
        selection: Option<Selection>,
    ) -> Result<(), Error> {
        self.apply(&ScenePatch {
            base_revision: self.revision(),
            operations: vec![PatchOperation::SetInteraction { channel, selection }],
        })
    }

    /// Sets an explicit semantic camera or restores automatic framing.
    ///
    /// # Errors
    ///
    /// Returns an error if the camera is non-finite or degenerate.
    pub fn set_camera(&mut self, camera: Option<molgfx_math::Camera>) -> Result<(), Error> {
        self.apply(&ScenePatch {
            base_revision: self.revision(),
            operations: vec![PatchOperation::SetCamera { camera }],
        })
    }

    /// Applies all operations or leaves the scene unchanged.
    ///
    /// # Errors
    ///
    /// Returns a revision conflict, missing-ID, validation, or renderer error.
    pub fn apply(&mut self, patch: &ScenePatch) -> Result<(), Error> {
        if patch.base_revision != self.spec.revision {
            return Err(PatchError::RevisionConflict {
                expected: patch.base_revision,
                actual: self.spec.revision,
            }
            .into());
        }
        let candidate = candidate_spec(&self.spec, patch)?;
        let requires_rebuild = patch.operations.iter().any(|operation| {
            matches!(
                operation,
                PatchOperation::AddRepresentation { .. }
                    | PatchOperation::RemoveRepresentation { .. }
                    | PatchOperation::ReplaceRepresentation { .. }
            )
        });
        if requires_rebuild {
            let resolution = resolve(&candidate, &self.structures).map_err(|error| {
                Error::InvalidSpec(format!("patch could not be resolved: {error}"))
            })?;
            self.resolved = resolution.scene;
            self.representations = resolution.representations;
            self.selections = resolution.selections;
        } else {
            apply_runtime_edits(
                &mut self.resolved,
                &self.representations,
                &candidate,
                &patch.operations,
            )?;
        }
        self.spec = candidate;
        self.next_representation = next_representation_id(&self.spec)?;
        Ok(())
    }

    /// Collects several semantic changes into one revision and patch.
    ///
    /// # Errors
    ///
    /// Returns the edit callback error or an atomic patch application error.
    pub fn transaction<F>(&mut self, edit: F) -> Result<ScenePatch, Error>
    where
        F: FnOnce(&mut SceneTransaction) -> Result<(), Error>,
    {
        let mut transaction = SceneTransaction {
            patch: ScenePatch::empty(self.revision()),
        };
        edit(&mut transaction)?;
        let patch = transaction.patch;
        self.apply(&patch)?;
        Ok(patch)
    }

    /// Deterministic semantic plan summary.
    #[must_use]
    pub fn explain(&self) -> String {
        let visible = self
            .spec
            .representations
            .values()
            .filter(|representation| representation.visible)
            .count();
        format!(
            "SceneSpec v{}\nrevision: {}\nstructures: {}\nrepresentations: {} ({} visible)\ncanonical selections: {}",
            self.spec.version,
            self.spec.revision,
            self.spec.structures.len(),
            self.spec.representations.len(),
            visible,
            canonical_selection_count(&self.spec)
        )
    }

    pub(crate) const fn resolved(&self) -> &molgfx_core::Scene {
        &self.resolved
    }

    /// Camera framing the current resolved molecular bounds.
    #[must_use]
    pub fn framing_camera(&self, aspect: f32) -> molgfx_math::Camera {
        let Some(mut camera) = self.spec.camera else {
            return molgfx_math::Camera::framing_aabb(&self.resolved.world_aabb(), aspect);
        };
        camera.projection.set_aspect(aspect);
        camera
    }
}

#[cfg(test)]
#[path = "scene_tests.rs"]
mod tests;
