//! Inward-facing adapter from `MolFrame`'s capsule ABI to renderer storage.

use molgfx::source::{
    AtomSelection, CoreError, MolecularProvider, SourceAtom, SourceBond, SourceTopology,
    topology_identity,
};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub(super) struct NativeProvider {
    source: molframe_py::NativeStructureSource,
    topology: SourceTopology,
    identity: u64,
}

impl NativeProvider {
    pub(super) fn import(object: &Bound<'_, PyAny>) -> PyResult<Self> {
        let source = molframe_py::NativeStructureSource::from_python(object)?;
        let native = source.topology();
        let mut atoms = Vec::with_capacity(native.atoms.len());
        for atom in native.atoms {
            let element = u8::try_from(atom.element)
                .map_err(|_| PyValueError::new_err("atomic number exceeds the renderer format"))?;
            atoms.push(SourceAtom {
                element,
                residue: atom.residue,
            });
        }
        let bonds = native
            .bonds
            .into_iter()
            .map(|bond| SourceBond {
                atoms: [bond.first, bond.second],
                aromatic: bond.aromatic != 0,
            })
            .collect::<Vec<_>>();
        let topology = SourceTopology {
            atoms: atoms.into(),
            residue_atom_start: native.residue_atom_start.into(),
            chain_residue_start: native.chain_residue_start.into(),
            model_chain_start: native.model_chain_start.into(),
            bonds: bonds.into(),
        };
        let identity = topology_identity(&topology);
        Ok(Self {
            source,
            topology,
            identity,
        })
    }

    pub(super) fn browser_bytes(&self) -> PyResult<Vec<u8>> {
        self.source.encode_bcif()
    }
}

impl MolecularProvider for NativeProvider {
    fn identity(&self) -> u64 {
        self.identity
    }

    fn coordinates(&self) -> &[[f32; 3]] {
        self.source.coordinates()
    }

    fn coordinate_revision(&self) -> u64 {
        self.source.coordinate_generation()
    }

    fn topology(&self) -> &SourceTopology {
        &self.topology
    }

    fn select(&self, source: &str) -> Result<AtomSelection, CoreError> {
        let rows = self
            .source
            .select(source)
            .map_err(|_| CoreError::InvalidSelection {
                reason: "MolFrame query evaluation failed",
            })?;
        Ok(compact(rows, self.coordinates().len()))
    }
}

/// Folds a provider row vector into the compactest selection shape.
fn compact(rows: Vec<u32>, atom_count: usize) -> AtomSelection {
    if rows.is_empty() {
        AtomSelection::Empty
    } else if rows.len() == atom_count {
        AtomSelection::All
    } else {
        AtomSelection::Sparse(rows)
    }
}

pub(super) fn source(provider: &NativeProvider) -> molgfx::source::MolecularSource {
    molgfx::source::MolecularSource::new(provider.clone())
}

pub(super) type SharedNativeProvider = Arc<NativeProvider>;
