//! Mutable semantic scene and atomic transaction boundary.

use crate::error::Error;
use crate::id::{RepresentationId, StructureId};
use crate::representation::{SceneItem, Selection};
use crate::scene::hashing::structure_hash;
use crate::scene::runtime::{next_representation_id, next_structure_id, resolve};
use crate::spec::{InteractionChannel, PatchOperation, ScenePatch, SceneSpec, StructureSource};
use molgfx_core::{RepresentationHandle, SelectionHandle};
use std::collections::BTreeMap;

/// Metadata for one residue observed in a bound molecular structure.
#[derive(Clone, Debug, serde::Serialize)]
pub struct ResidueMetadata {
    /// Chain identifier.
    pub chain: Option<String>,
    /// Author-provided chain identifier.
    pub auth_chain: Option<String>,
    /// Entity identifier.
    pub entity: Option<u32>,
    /// One-letter residue code.
    pub one_letter: Option<String>,
    /// Component identifier.
    pub component: Option<String>,
    /// Author-provided component identifier.
    pub auth_component: Option<String>,
    /// Author-provided residue number.
    pub auth_number: Option<i32>,
    /// Label residue number.
    pub label_number: Option<i32>,
    /// Insertion code.
    pub insertion_code: Option<String>,
    /// Whether the residue was observed.
    pub observed: bool,
    /// Zero-based residue index.
    pub residue_index: u32,
    /// Inclusive atom range.
    pub atom_range: Option<[u32; 2]>,
}
/// A molecular atom resolved against the exact structure and topology owned by a scene.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResolvedAtomPick {
    /// Structure identifier.
    pub structure: StructureId,
    /// Dataset identifier.
    pub dataset: u64,
    /// Chunk identifier.
    pub chunk: u64,
    /// Topology revision identifier.
    pub topology_revision: u64,
    /// Atom index within the topology.
    pub atom_index: u32,
}

/// Result of resolving a renderer pick against a live semantic scene.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResolvedPick {
    /// An atom resolved to stable scene coordinates.
    Atom(ResolvedAtomPick),
    /// A non-atom renderer pick.
    NonAtom(crate::PickResult),
}

/// Mutable scene state backed by one resolved renderer scene.
#[derive(Debug)]
pub struct Scene {
    spec: SceneSpec,
    resolved: molgfx_core::Scene,
    structures: BTreeMap<StructureId, molgfx_core::MolecularSource>,
    representations: BTreeMap<RepresentationId, RepresentationHandle>,
    selections: BTreeMap<String, SelectionHandle>,
    visuals: BTreeMap<RepresentationId, crate::visual::ResolvedVisual>,
    property_bindings: BTreeMap<Box<str>, crate::ScalarPropertyBinding>,
    properties: BTreeMap<Box<str>, molgfx_core::AtomPropertyHandle>,
    science_bindings: crate::science::ScienceBindings,
    science: crate::science::lower::LoweredScience,
    /// Structure assets of the current resolution, so a later resolution over
    /// the same sources reuses their atom tables instead of rebuilding them.
    structure_assets: crate::scene::runtime::StructureAssets,
    /// Evaluated rows by query and molecule identity, shared by every edit.
    rows: crate::scene::selection_rows::SelectionRows,
    /// Each structure's appearance class column, for structures with rules.
    appearance: crate::scene::appearance::AppearanceColumns,
    next_structure: u64,
    next_representation: u64,
}

pub(crate) struct Resolution {
    pub(crate) scene: molgfx_core::Scene,
    pub(crate) representations: BTreeMap<RepresentationId, RepresentationHandle>,
    pub(crate) selections: BTreeMap<String, SelectionHandle>,
    pub(crate) visuals: BTreeMap<RepresentationId, crate::visual::ResolvedVisual>,
    pub(crate) properties: BTreeMap<Box<str>, molgfx_core::AtomPropertyHandle>,
    pub(crate) science: crate::science::lower::LoweredScience,
    pub(crate) appearance: crate::scene::appearance::AppearanceColumns,
}

pub(crate) mod appearance;
pub(crate) mod domains;
pub(crate) mod hashing;
mod insertion;
pub(crate) mod interaction;
#[cfg(test)]
mod interaction_tests;
mod operations;
pub(crate) mod properties;
pub(crate) mod runtime;
#[cfg(test)]
mod runtime_tests;
mod science;
pub(crate) mod selection_rows;
pub(crate) mod transaction;

impl Scene {
    /// Resolves a physical pick against this scene's exact dataset and topology.
    ///
    /// The dataset-to-structure mapping is derived explicitly from each placed
    /// structure's authoritative metadata: a placed source proves its semantic
    /// owner by shared storage with exactly one bound structure. A dataset that
    /// maps to several structures is rejected as ambiguous, and one that maps
    /// to none is rejected as stale, instead of trusting positional order.
    ///
    /// # Errors
    ///
    /// Returns [`Error::AmbiguousPick`] when a dataset or source identity maps
    /// to multiple structures, [`Error::StalePick`] when no bound structure
    /// proves ownership, and [`Error::InvalidSpec`] for malformed picks.
    pub fn resolve_pick(&self, pick: &crate::PickResult) -> Result<ResolvedPick, Error> {
        let Some(dataset) = pick.dataset else {
            return Ok(ResolvedPick::NonAtom(pick.clone()));
        };
        let Some(chunk) = pick.chunk else {
            return Err(Error::InvalidSpec(
                "pick is missing its chunk identity".to_owned(),
            ));
        };
        let Some(row) = pick.row else {
            return Err(Error::InvalidSpec(
                "pick is missing its logical row".to_owned(),
            ));
        };
        let mut placed = None;
        for candidate in self.resolved.structures().map(|(_, placed)| placed) {
            if candidate.dataset_id().get() != dataset {
                continue;
            }
            if placed.is_some() {
                return Err(Error::AmbiguousPick { dataset });
            }
            placed = Some(candidate);
        }
        let placed = placed.ok_or(Error::StalePick { dataset })?;
        let structure = self.structure_for_source(&placed.source, dataset)?;
        let atom_index = u32::try_from(row).map_err(|_| {
            Error::InvalidSpec(format!("pick row {row} is outside the atom index space"))
        })?;
        if pick.kind == crate::PickKind::Atom
            && usize::try_from(atom_index)
                .ok()
                .is_none_or(|index| index >= placed.atoms.len() as usize)
        {
            return Err(Error::InvalidSpec(format!(
                "pick row {row} is outside structure topology"
            )));
        }
        if pick.kind == crate::PickKind::Atom {
            Ok(ResolvedPick::Atom(ResolvedAtomPick {
                structure,
                dataset,
                chunk,
                topology_revision: self.resolved.structure_revision(),
                atom_index,
            }))
        } else {
            Ok(ResolvedPick::NonAtom(pick.clone()))
        }
    }
    /// Resolves the semantic structure that owns one placed physical source.
    ///
    /// Ownership is proven by shared provider storage with a bound structure,
    /// never by iteration position. Two bindings sharing one source cannot be
    /// told apart, so the mapping is ambiguous rather than positional.
    fn structure_for_source(
        &self,
        source: &molgfx_core::MolecularSource,
        dataset: u64,
    ) -> Result<StructureId, Error> {
        let mut owner = None;
        for (identity, bound) in &self.structures {
            if !bound.shares_storage_with(source) {
                continue;
            }
            if owner.is_some() {
                return Err(Error::AmbiguousPick { dataset });
            }
            owner = Some(*identity);
        }
        owner.ok_or(Error::StalePick { dataset })
    }
    /// Returns typed metadata for residues observed in one structure.
    ///
    /// # Errors
    ///
    /// Returns an error when the structure is unknown, is not backed by
    /// `MolFrame` data, or its residue index space cannot be represented.
    pub fn residue_metadata(&self, structure: StructureId) -> Result<Vec<ResidueMetadata>, Error> {
        let source = self
            .structures
            .get(&structure)
            .ok_or_else(|| Error::InvalidSpec(format!("unknown structure {}", structure.get())))?;
        let Some(input) = source.molframe() else {
            return Err(Error::InvalidSpec(
                "residue metadata requires a MolFrame-backed structure".to_owned(),
            ));
        };
        let topology = source.topology();
        let mut metadata = Vec::with_capacity(input.residue_count());
        for (chain_index, chain) in input.chains().iter().enumerate() {
            let chain_start = topology
                .chain_residue_start
                .get(chain_index)
                .copied()
                .unwrap_or_default();
            for (local_residue, residue) in chain.residues().enumerate() {
                let local_residue = u32::try_from(local_residue).map_err(|_| {
                    Error::InvalidSpec("residue index exceeds the supported range".to_owned())
                })?;
                let residue_index = chain_start.checked_add(local_residue).ok_or_else(|| {
                    Error::InvalidSpec("residue index exceeds the supported range".to_owned())
                })?;
                let atom_range = topology
                    .residue_atom_start
                    .get(residue_index as usize..residue_index as usize + 2)
                    .and_then(|range| (range.len() == 2).then_some([range[0], range[1]]));
                metadata.push(ResidueMetadata {
                    chain: chain.label().map(str::to_owned),
                    auth_chain: chain.auth_label().map(str::to_owned),
                    entity: chain.entity().map(molframe::EntityIndex::get),
                    one_letter: None,
                    component: residue.name().map(str::to_owned),
                    auth_component: residue.auth_name().map(str::to_owned),
                    auth_number: residue.auth_seq_id(),
                    label_number: residue.label_seq_id(),
                    insertion_code: residue.ins_code().map(str::to_owned),
                    observed: true,
                    residue_index,
                    atom_range,
                });
            }
        }
        Ok(metadata)
    }
    /// Builds a scene over a shared immutable `MolFrame` snapshot.
    ///
    /// # Errors
    ///
    /// Returns a validation error if the molecular source cannot be adapted.
    pub fn from_structure(structure: &molframe::Structure) -> Result<Self, Error> {
        Self::from_source(molgfx_core::MolecularSource::from_molframe(structure))
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
            science_bindings: crate::science::ScienceBindings::default(),
            structure_assets: crate::scene::runtime::StructureAssets::default(),
            science: crate::science::lower::LoweredScience::default(),
            rows: crate::scene::selection_rows::SelectionRows::default(),
            appearance: BTreeMap::new(),
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
        let science_bindings = crate::science::ScienceBindings::default();
        let rows = crate::scene::selection_rows::SelectionRows::default();
        let resolution = resolve(
            &spec,
            &structures,
            &property_bindings,
            &science_bindings,
            &rows,
        )?;
        let next_structure = next_structure_id(&spec)?;
        let next_representation = next_representation_id(&spec)?;
        let structure_assets =
            crate::scene::runtime::StructureAssets::capture(&structures, &resolution.scene);
        Ok(Self {
            spec,
            resolved: resolution.scene,
            structures,
            representations: resolution.representations,
            selections: resolution.selections,
            visuals: resolution.visuals,
            property_bindings,
            properties: resolution.properties,
            science_bindings,
            science: resolution.science,
            structure_assets,
            rows,
            appearance: resolution.appearance,
            next_structure,
            next_representation,
        })
    }

    /// Adds one immutable representation and returns its semantic identity.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid selection or representation value.
    pub fn add<T: SceneItem>(&mut self, item: T) -> Result<T::Id, Error> {
        item.add_to(self)
    }

    pub(crate) fn insert_representation(
        &mut self,
        mut representation: crate::RepresentationSpec,
    ) -> Result<RepresentationId, Error> {
        if representation.common.structure.is_none() {
            if self.structures.len() != 1 {
                return Err(Error::InvalidSpec(
                    "multi-structure scenes require an explicit representation structure"
                        .to_owned(),
                ));
            }
            representation.common.structure = self.structures.first_key_value().map(|(id, _)| *id);
        }
        let id = RepresentationId(self.next_representation);
        self.apply(&ScenePatch {
            base_revision: self.revision(),
            operations: vec![PatchOperation::AddRepresentation { id, representation }],
        })?;
        Ok(id)
    }

    /// Adds another shared `MolFrame` snapshot without copying coordinates.
    ///
    /// # Errors
    ///
    /// Returns an error if the source cannot be adapted or IDs are exhausted.
    pub fn add_structure(&mut self, structure: &molframe::Structure) -> Result<StructureId, Error> {
        self.add_source(&molgfx_core::MolecularSource::from_molframe(structure))
    }

    /// Adds another provider-neutral source without copying coordinates.
    ///
    /// Announces the structure through an atomic `AddStructure` patch so the
    /// patch stream reaches remote scenes and browser viewers. The molecular
    /// data stays in the local binding map; the patch carries only the portable
    /// descriptor.
    ///
    /// # Errors
    ///
    /// Returns an error if the source cannot be adapted or IDs are exhausted.
    pub fn add_source(
        &mut self,
        source: &molgfx_core::MolecularSource,
    ) -> Result<StructureId, Error> {
        let id = StructureId(self.next_structure);
        let next_structure = self.next_structure.checked_add(1).ok_or_else(|| {
            Error::InvalidSpec("structure identity space is exhausted".to_owned())
        })?;
        let patch = ScenePatch {
            base_revision: self.revision(),
            operations: vec![PatchOperation::AddStructure {
                id,
                source: StructureSource {
                    content_hash: structure_hash(source),
                    uri: None,
                    format: None,
                },
            }],
        };
        // The patch plan resolves against this binding map, so the source must
        // be present while the structural patch is prepared and committed.
        let mut bound = std::mem::take(&mut self.structures);
        let _ = bound.insert(id, source.clone());
        let previous = std::mem::replace(&mut self.structures, bound);
        if let Err(error) = self.apply(&patch) {
            self.structures = previous;
            return Err(error);
        }
        self.next_structure = next_structure;
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

    /// Focuses a molecular subject and de-emphasizes its distant context.
    ///
    /// The selected atoms receive the focused interaction bit, and — unless the
    /// muted channel has been set explicitly — whole residues beyond the
    /// surrounding shell receive the muted bit. Both are ordinary state bits, so
    /// a visual style reads them through `visual.state(..)` and the camera frames
    /// them through [`Self::focus_bounds`]. No hidden representation is inserted.
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
}

#[cfg(test)]
mod tests;
