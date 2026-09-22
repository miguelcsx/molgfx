//! Binding caller-owned molecular storage to a scene.
//!
//! A host language already owns the molecular data — `MolFrame` in Python, the
//! browser's transferred buffers in WASM — and `MolGFX` reads those coordinates
//! where they already live rather than copying them into a scene. A binding
//! implements [`MolecularProvider`] over its own storage and hands the result
//! to [`crate::Scene::from_source`].
//!
//! This is the seam for language bindings and embedders, not part of the
//! ordinary authoring vocabulary: callers who already have a `MolFrame`
//! structure use [`crate::Scene::from_structure`] instead.

pub use molgfx_core::{
    AtomSelection, CoreError, MolecularProvider, MolecularSource, SourceAtom, SourceBond,
    SourceTopology,
};

/// A content-derived identity for a provider's molecular topology.
///
/// Providers must report an identity that is equal exactly when two sources
/// describe the same molecule, because the renderer shares GPU assets between
/// sources that agree. Addresses are the wrong answer: a coordinate buffer can
/// move when a structure is edited, and two handles onto one structure can hold
/// different buffers. Topology does not move, so it is what identity is taken
/// from — and it stays stable while coordinates animate.
#[must_use]
pub fn topology_identity(topology: &SourceTopology) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    topology.atoms.len().hash(&mut hasher);
    for atom in topology.atoms.iter() {
        atom.element.hash(&mut hasher);
        atom.residue.hash(&mut hasher);
    }
    topology.residue_atom_start.hash(&mut hasher);
    topology.chain_residue_start.hash(&mut hasher);
    topology.model_chain_start.hash(&mut hasher);
    for bond in topology.bonds.iter() {
        bond.atoms.hash(&mut hasher);
        bond.aromatic.hash(&mut hasher);
    }
    hasher.finish()
}
