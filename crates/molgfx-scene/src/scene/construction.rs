//! Building a scene from a structure, a source or a specification.

use super::{Scene, with_derived_bindings};
use crate::error::Error;
use crate::id::StructureId;
use crate::scene::hashing::structure_hash;
use crate::scene::runtime::{next_representation_id, next_structure_id, resolve};
use crate::spec::{SceneSpec, StructureSource};
use std::collections::BTreeMap;

impl Scene {
    /// Creates a resolved scene without any molecular structure.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            spec: SceneSpec::empty(),
            resolved: molgfx_core::Scene::new(),
            structures: BTreeMap::new(),
            representations: BTreeMap::new(),
            selections: BTreeMap::new(),
            visuals: BTreeMap::new(),
            property_bindings: BTreeMap::new(),
            properties: BTreeMap::new(),
            overlay_bindings: crate::overlay::OverlayBindings::default(),
            structure_assets: crate::scene::runtime::StructureAssets::default(),
            overlay: crate::overlay::lower::LoweredOverlay::default(),
            rows: crate::scene::selection_rows::SelectionRows::default(),
            appearance: BTreeMap::new(),
            next_structure: 1,
            next_representation: 1,
            snapshot_bindings: super::snapshot::SnapshotBindings::default(),
            playback: BTreeMap::new(),
            selected_scope: None,
        }
    }
    /// Builds a scene over a shared immutable `MolFrame` snapshot.
    ///
    /// A multi-model structure draws its first model; use
    /// [`Self::from_structure_model`] for another. The structure is bound as
    /// `StructureId(1)` (see [`Self::from_structure_with_id`]).
    ///
    /// # Errors
    ///
    /// Returns a validation error if the molecular source cannot be adapted
    /// or if the structure has no atoms.
    pub fn from_structure(structure: &molframe::Structure) -> Result<Self, Error> {
        Self::from_structure_model(structure, 0)
    }

    /// Builds a scene over a provider-neutral immutable molecular source.
    ///
    /// # Errors
    ///
    /// Returns a validation error if the molecular source cannot be adapted.
    pub fn from_source(source: molgfx_core::MolecularSource) -> Result<Self, Error> {
        let structure_id = StructureId(1);
        let mut spec = SceneSpec::empty();
        let _ = spec.structures.insert(
            structure_id,
            StructureSource {
                content_hash: structure_hash(&source),
                uri: None,
                format: None,
                placement: None,
            },
        );
        let mut structures = BTreeMap::new();
        let _ = structures.insert(structure_id, source.clone());
        let resolved = molgfx_core::Scene::from_source(source)?;
        Ok(Self {
            spec,
            resolved,
            structures,
            representations: BTreeMap::new(),
            selections: BTreeMap::new(),
            visuals: BTreeMap::new(),
            property_bindings: BTreeMap::new(),
            properties: BTreeMap::new(),
            overlay_bindings: crate::overlay::OverlayBindings::default(),
            structure_assets: crate::scene::runtime::StructureAssets::default(),
            overlay: crate::overlay::lower::LoweredOverlay::default(),
            rows: crate::scene::selection_rows::SelectionRows::default(),
            appearance: BTreeMap::new(),
            next_structure: 2,
            next_representation: 1,
            snapshot_bindings: super::snapshot::SnapshotBindings::default(),
            playback: BTreeMap::new(),
            selected_scope: None,
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
        if let Some(identity) = structures
            .iter()
            .find_map(|(id, structure)| structure.is_ragged_ensemble().then_some(id))
        {
            return Err(Error::InvalidSpec(format!(
                "structure {} is a ragged multi-model ensemble with no atoms of its own; \
                 bind one model from Scene::model_structure",
                identity.get()
            )));
        }
        Self::from_spec_with_properties(
            spec,
            structures
                .into_iter()
                .map(|(id, structure)| {
                    (id, molgfx_core::MolecularSource::from_molframe(&structure))
                })
                .collect(),
            BTreeMap::new(),
        )
    }

    /// Resolves a portable specification with explicit shared property columns.
    ///
    /// # Errors
    ///
    /// Returns an error unless structure and property bindings exactly match
    /// their descriptors and every value column satisfies its row contract.
    pub fn from_spec_with_properties(
        spec: SceneSpec,
        structures: BTreeMap<StructureId, molgfx_core::MolecularSource>,
        property_bindings: BTreeMap<Box<str>, crate::ScalarPropertyBinding>,
    ) -> Result<Self, Error> {
        Self::from_spec_with_bindings(
            spec,
            structures,
            property_bindings,
            super::RuntimeBindings::default(),
        )
    }

    /// Restores an authored scene and its shared runtime assets in one resolution.
    /// No binding increments the restored document's semantic revision.
    ///
    /// # Errors
    /// Rejects inconsistent molecular, property or scientific asset bindings.
    pub fn from_spec_with_bindings(
        spec: SceneSpec,
        structures: BTreeMap<StructureId, molgfx_core::MolecularSource>,
        property_bindings: BTreeMap<Box<str>, crate::ScalarPropertyBinding>,
        mut bindings: super::RuntimeBindings,
    ) -> Result<Self, Error> {
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
        let property_bindings = with_derived_bindings(&spec, &structures, property_bindings)?;
        let playback = std::mem::take(&mut bindings.playback);
        let overlay_bindings = bindings.into_overlay(&spec, &structures)?;
        let rows = crate::scene::selection_rows::SelectionRows::default();
        let resolution = resolve(
            &spec,
            &structures,
            &property_bindings,
            &overlay_bindings,
            &rows,
        )?;
        let next_structure = next_structure_id(&spec)?;
        let next_representation = next_representation_id(&spec)?;
        let structure_assets =
            crate::scene::runtime::StructureAssets::capture(&structures, &resolution.scene);
        let mut scene = Self {
            spec,
            resolved: resolution.scene,
            structures,
            representations: resolution.representations,
            selections: resolution.selections,
            visuals: resolution.visuals,
            property_bindings,
            properties: resolution.properties,
            overlay_bindings,
            overlay: resolution.overlay,
            structure_assets,
            rows,
            appearance: resolution.appearance,
            next_structure,
            next_representation,
            snapshot_bindings: super::snapshot::SnapshotBindings::default(),
            playback: BTreeMap::new(),
            selected_scope: None,
        };
        let mut restored = std::collections::BTreeSet::new();
        for binding in playback {
            if !restored.insert(binding.structure) {
                return Err(Error::InvalidSpec(
                    "duplicate molecular playback binding".to_owned(),
                ));
            }
            scene.set_trajectory_pair(
                binding.structure,
                binding.start,
                binding.end,
                binding.sample_seconds,
            )?;
        }
        Ok(scene)
    }
}
