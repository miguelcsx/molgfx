//! Mutable semantic scene and atomic transaction boundary.

use crate::error::Error;
use crate::id::{RepresentationId, StructureId};
use crate::representation::{SceneItem, Selection};
use crate::scene::runtime::{next_representation_id, next_structure_id, resolve, structure_hash};
use crate::spec::{InteractionChannel, PatchOperation, ScenePatch, SceneSpec, StructureSource};
use molgfx_core::{RepresentationHandle, SelectionHandle};
use std::collections::BTreeMap;

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
}

pub(crate) mod interaction;
#[cfg(test)]
mod interaction_tests;
mod operations;
pub(crate) mod properties;
pub(crate) mod runtime;
#[cfg(test)]
mod runtime_tests;
mod science;
pub(crate) mod transaction;

impl Scene {
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
        let resolution = resolve(&spec, &structures, &property_bindings, &science_bindings)?;
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

    pub(crate) fn insert_volume(
        &mut self,
        volume: crate::VolumeSpec,
    ) -> Result<crate::VolumeId, Error> {
        let id = crate::VolumeId(next_id_for(
            self.spec.volumes.last_key_value().map(|(id, _)| id.get()),
            "volume",
        )?);
        self.apply(&ScenePatch {
            base_revision: self.revision(),
            operations: vec![PatchOperation::AddVolume { id, volume }],
        })?;
        Ok(id)
    }

    pub(crate) fn insert_annotation(
        &mut self,
        annotation: crate::AnnotationSpec,
    ) -> Result<crate::AnnotationId, Error> {
        let id = crate::AnnotationId(next_id_for(
            self.spec
                .annotations
                .last_key_value()
                .map(|(id, _)| id.get()),
            "annotation",
        )?);
        self.apply(&ScenePatch {
            base_revision: self.revision(),
            operations: vec![PatchOperation::AddAnnotation { id, annotation }],
        })?;
        Ok(id)
    }

    pub(crate) fn insert_measurement(
        &mut self,
        measurement: crate::MeasurementSpec,
    ) -> Result<crate::MeasurementId, Error> {
        let id = crate::MeasurementId(next_id_for(
            self.spec
                .measurements
                .last_key_value()
                .map(|(id, _)| id.get()),
            "measurement",
        )?);
        self.apply(&ScenePatch {
            base_revision: self.revision(),
            operations: vec![PatchOperation::AddMeasurement { id, measurement }],
        })?;
        Ok(id)
    }

    pub(crate) fn insert_scientific_interaction(
        &mut self,
        interaction: crate::ScientificInteractionSpec,
    ) -> Result<crate::ScientificInteractionId, Error> {
        let id = crate::ScientificInteractionId(next_id_for(
            self.spec
                .scientific_interactions
                .last_key_value()
                .map(|(id, _)| id.get()),
            "scientific interaction",
        )?);
        self.apply(&ScenePatch {
            base_revision: self.revision(),
            operations: vec![PatchOperation::AddScientificInteraction { id, interaction }],
        })?;
        Ok(id)
    }

    pub(crate) fn insert_trajectory(
        &mut self,
        trajectory: crate::TrajectorySpec,
    ) -> Result<crate::TrajectoryId, Error> {
        let id = crate::TrajectoryId(next_id_for(
            self.spec
                .trajectories
                .last_key_value()
                .map(|(id, _)| id.get()),
            "trajectory",
        )?);
        self.apply(&ScenePatch {
            base_revision: self.revision(),
            operations: vec![PatchOperation::AddTrajectory { id, trajectory }],
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

fn next_id_for(last: Option<u64>, kind: &str) -> Result<u64, Error> {
    crate::fallback(last, 0)
        .checked_add(1)
        .ok_or_else(|| Error::InvalidSpec(format!("{kind} identity space is exhausted")))
}

#[cfg(test)]
mod tests;
