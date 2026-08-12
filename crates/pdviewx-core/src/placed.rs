//! A structure placed in a scene.
//!
//! Placement pairs the immutable parsed structure with a model transform and
//! the scene's per-atom tables. Multiple placements of the same structure
//! share its storage; each carries its own transform and columns.

use crate::atoms::AtomTable;
use crate::hierarchy::Hierarchy;
use pdviewx_math::{Aabb, Mat4};

/// One structure in the scene, with its transform and derived tables.
#[derive(Clone, Debug)]
pub struct PlacedStructure {
    /// The parsed structure; a cheap reference-counted handle.
    pub structure: pdbiox::Structure,
    /// Model-to-world transform.
    pub model_to_world: Mat4,
    /// The dense per-atom table for the placed model.
    pub atoms: AtomTable,
    /// Offset-array hierarchy over the structure's topology.
    pub hierarchy: Hierarchy,
}

impl PlacedStructure {
    /// Places the first model of a structure at the identity transform.
    #[must_use]
    pub fn new(structure: &pdbiox::Structure) -> Option<Self> {
        let model = pdbiox::ModelIndex::new(0);
        let atoms = AtomTable::from_structure(structure, model)?;
        Some(Self {
            structure: structure.clone(),
            model_to_world: Mat4::IDENTITY,
            hierarchy: Hierarchy::from_structure(structure),
            atoms,
        })
    }

    /// World-space bound of the placed coordinates, `O(atoms)`.
    #[must_use]
    pub fn world_aabb(&self) -> Aabb {
        self.atoms.coords().aabb().transform(&self.model_to_world)
    }
}
