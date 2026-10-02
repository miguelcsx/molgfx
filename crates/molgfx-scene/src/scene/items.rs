//! Adding representations, overlays and structures to a scene.

use super::Scene;
use crate::error::Error;
use crate::id::{RepresentationId, StructureId};
use crate::representation::SceneItem;
use crate::spec::{PatchOperation, ScenePatch};

impl Scene {
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
}
