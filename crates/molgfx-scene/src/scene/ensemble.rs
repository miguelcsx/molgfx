//! Multi-model structures: choosing which model a scene draws.
//!
//! A deposited ensemble (NMR, for instance) is either *dense*, every model
//! sharing one topology, or *ragged*, models with differing atom counts. A
//! ragged ensemble is an outer shell with no atoms of its own, so drawing it
//! directly would produce an empty image. A scene always draws exactly one
//! model; the rest are reached by building the scene from another index.

use super::Scene;
use crate::error::Error;
use crate::id::StructureId;

/// The identity `from_structure` and `from_source` give their one structure.
const FIRST_STRUCTURE: StructureId = StructureId(1);

impl Scene {
    /// Builds a scene over one model of a possibly multi-model structure.
    ///
    /// `model` is a zero-based position. A single-model structure only has
    /// model 0. Dense ensembles share topology; a later model's coordinates
    /// replace the first model's in the drawn snapshot. Ragged ensembles hold
    /// independent structures and cannot be interpolated between models.
    ///
    /// # Errors
    ///
    /// Returns an error when `model` is out of range or the chosen model has
    /// no atoms.
    pub fn from_structure_model(
        structure: &molframe::Structure,
        model: usize,
    ) -> Result<Self, Error> {
        let chosen = Self::model_structure(structure, model)?;
        Self::from_structure_checked(&chosen)
    }

    /// Returns the single-model structure for one model of `structure`.
    ///
    /// # Errors
    ///
    /// Returns an error when `model` is out of range, or when a dense model
    /// cannot be materialized.
    pub fn model_structure(
        structure: &molframe::Structure,
        model: usize,
    ) -> Result<molframe::Structure, Error> {
        let count = structure.model_count();
        if let Some(models) = structure.ragged_models() {
            let available = models.len();
            return models.into_iter().nth(model).ok_or_else(|| {
                Error::InvalidSpec(format!(
                    "model {model} is out of range: the structure has {available} models (0-based)"
                ))
            });
        }
        if model >= count.max(1) {
            return Err(Error::InvalidSpec(format!(
                "model {model} is out of range: the structure has {count} models (0-based)"
            )));
        }
        if model == 0 {
            return Ok(structure.clone());
        }
        let index = |position: usize| {
            u32::try_from(position)
                .map(molframe::ModelIndex::new)
                .map_err(|_| Error::InvalidSpec(format!("model {position} is out of range")))
        };
        let frame = structure
            .model_coordinates(index(model)?)
            .ok_or_else(|| Error::InvalidSpec(format!("model {model} has no coordinates")))?
            .to_vec();
        let invalid = |reason: String| Error::InvalidSpec(reason);
        let mut editor = structure
            .engine()
            .edit_coordinates(&molframe::ExecutionContext::default())
            .map_err(|error| invalid(format!("model {model} cannot be materialized: {error}")))?;
        let target = editor
            .positions_mut(index(0)?)
            .ok_or_else(|| invalid("model 0 is not editable".to_owned()))?;
        target.copy_from_slice(&frame);
        editor
            .commit()
            .map(molframe::Structure::from)
            .map_err(|_| invalid(format!("model {model} failed structural validation")))
    }

    /// Identities of every structure in the scene, in ascending order.
    pub fn structure_ids(&self) -> impl Iterator<Item = StructureId> + '_ {
        self.spec.structures.keys().copied()
    }

    /// The first structure of the scene, the one `from_structure` creates.
    #[must_use]
    pub fn primary_structure(&self) -> Option<StructureId> {
        self.spec.structures.keys().next().copied()
    }

    /// Builds a scene like [`Self::from_structure`] and also returns the
    /// identity of its structure, ready to pass to [`Self::place`].
    ///
    /// # Errors
    ///
    /// Same as [`Self::from_structure`].
    pub fn from_structure_with_id(
        structure: &molframe::Structure,
    ) -> Result<(Self, StructureId), Error> {
        Self::from_structure(structure).map(|scene| (scene, FIRST_STRUCTURE))
    }

    /// Builds the scene for an already single-model structure and refuses an
    /// empty one.
    pub(super) fn from_structure_checked(structure: &molframe::Structure) -> Result<Self, Error> {
        let scene = Self::from_source(molgfx_core::MolecularSource::from_molframe(structure))?;
        scene.require_atoms()?;
        Ok(scene)
    }

    /// Refuses a scene that has no atoms: it could only render blank.
    pub(super) fn require_atoms(&self) -> Result<(), Error> {
        let empty = self
            .structures
            .values()
            .all(|source| source.coordinates().is_empty());
        if empty {
            return Err(Error::InvalidSpec(
                "the structure has no atoms, so nothing can be drawn; if it is a multi-model \
                 file, pick one model with Scene::from_structure_model"
                    .to_owned(),
            ));
        }
        Ok(())
    }
}
