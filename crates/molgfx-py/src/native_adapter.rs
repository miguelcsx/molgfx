//! Inward-facing adapter from `MolFrame`'s capsule ABI to renderer storage.

use molgfx::source::{
    AtomSelection, CoreError, MolecularProvider, SourceAtom, SourceBond, SourceTopology, Structure,
    topology_identity,
};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use std::sync::{Arc, OnceLock};

#[derive(Clone, Debug)]
pub(super) struct NativeProvider {
    source: molframe_py::NativeStructureSource,
    topology: SourceTopology,
    identity: u64,
    /// The structure this provider's own transported payload decodes to.
    ///
    /// A scene's content identity is recomputed by whoever receives the
    /// molecule, so the producer must digest the same structure the consumer
    /// builds. The capsule deliberately carries no Rust values across the
    /// extension boundary, so the only view both sides share is the payload
    /// itself; decoding it here — once, lazily — is what keeps the published
    /// descriptor verifiable by its own consumer.
    decoded: Arc<OnceLock<Option<Structure>>>,
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
            decoded: Arc::new(OnceLock::new()),
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

    /// The structure this provider's own payload decodes to.
    ///
    /// The capsule carries no Rust values across the extension boundary, so the
    /// consumer re-parses the transferred bytes while the producer cannot see
    /// its own structure directly. Decoding the payload here — once, lazily —
    /// gives both sides the same molecule to digest, which is what makes the
    /// published content identity verifiable by its own consumer.
    fn molframe(&self) -> Option<&Structure> {
        self.decoded
            .get_or_init(|| {
                self.browser_bytes().ok().and_then(|bytes| {
                    molgfx::source::structure_from_payload(&bytes, "structure.bcif")
                })
            })
            .as_ref()
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
