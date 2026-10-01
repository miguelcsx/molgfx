//! Binding molecular sources into a scene, alone or placed as copies.

use super::Scene;
use crate::error::Error;
use crate::id::StructureId;
use crate::scene::hashing::structure_hash;
use crate::spec::{PatchOperation, ScenePatch, StructureSource};

impl Scene {
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
        self.add_structure_source(source, None)
    }

    /// Adds a copy of `of`'s molecular source at `placement`, sharing its data.
    ///
    /// The copy is a structure in its own right — its own identity, selections,
    /// representations and picks — that reads the same coordinates through a
    /// column-major 4×4 affine matrix. An assembly's copies are described this
    /// way.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown structure, a non-finite, non-affine or
    /// singular matrix, or an exhausted identity space.
    pub fn place(&mut self, of: StructureId, placement: [f32; 16]) -> Result<StructureId, Error> {
        StructureSource::validate_placement(&placement)?;
        let source = self.structures.get(&of).cloned().ok_or_else(|| {
            Error::InvalidSpec(format!("structure {} is not part of this scene", of.0))
        })?;
        self.add_structure_source(&source, Some(placement))
    }

    fn add_structure_source(
        &mut self,
        source: &molgfx_core::MolecularSource,
        placement: Option<[f32; 16]>,
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
                    placement,
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
}
