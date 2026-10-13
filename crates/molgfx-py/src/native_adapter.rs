//! Inward-facing adapter from `MolFrame`'s capsule ABI to renderer storage.

use molgfx::source::{
    AtomSelection, CoreError, MolecularProvider, SourceAtom, SourceBond, SourceTopology, Structure,
    topology_identity,
};
use numpy::PyReadonlyArray1;
use pyo3::exceptions::{PyAttributeError, PyValueError};
use pyo3::prelude::*;
use std::sync::Arc;

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
    /// itself; decoding it here — once, at import — is what keeps the published
    /// descriptor verifiable by its own consumer. An encoding or decoding
    /// failure is raised by the import with its real cause, never hidden.
    decoded: Arc<Structure>,
}

impl NativeProvider {
    pub(super) fn import(object: &Bound<'_, PyAny>) -> PyResult<Self> {
        let source = molframe_py::NativeStructureSource::from_python(object)?;
        let native = source.topology();
        let secondary_structure = object
            .call_method0("_molframe_secondary_structure")
            .map_err(|error| {
                if error.is_instance_of::<PyAttributeError>(object.py()) {
                    PyAttributeError::new_err(
                        "the molframe Structure has no secondary-structure accessor; \
                         molgfx requires molframe>=0.5.0",
                    )
                } else {
                    error
                }
            })?
            .extract::<Vec<u8>>()?
            .into_iter()
            .map(decode_secondary_structure)
            .collect::<PyResult<Vec<_>>>()?;
        let mut atoms = Vec::with_capacity(native.atoms.len());
        let altloc_array = object.call_method0("_molframe_altlocs")?;
        let altlocs = altloc_array.extract::<PyReadonlyArray1<'_, u32>>()?;
        let altlocs = altlocs.as_slice()?;
        if altlocs.len() != native.atoms.len() {
            return Err(PyValueError::new_err(
                "alternate-conformation and atom columns differ",
            ));
        }
        for (atom, &altloc) in native.atoms.into_iter().zip(altlocs) {
            let element = u8::try_from(atom.element)
                .map_err(|_| PyValueError::new_err("atomic number exceeds the renderer format"))?;
            atoms.push(SourceAtom {
                element,
                residue: atom.residue,
                alt_id: molgfx::molframe::AltId::from_raw(altloc),
            });
        }
        let bonds = native
            .bonds
            .into_iter()
            .map(|bond| SourceBond {
                atoms: [bond.first, bond.second],
                order: decode_bond_order(bond.order, bond.aromatic != 0),
                aromatic: bond.aromatic != 0,
                metal: bond.metal_coordination != 0,
            })
            .collect::<Vec<_>>();
        let topology = SourceTopology {
            atoms: atoms.into(),
            residue_atom_start: native.residue_atom_start.into(),
            chain_residue_start: native.chain_residue_start.into(),
            model_chain_start: native.model_chain_start.into(),
            bonds: bonds.into(),
            // The Python provider does not yet forward anisotropic tensors, so
            // an imported structure is isotropic until it does.
            anisotropy: Vec::new().into(),
            secondary_structure: secondary_structure.into(),
        };
        let identity = topology_identity(&topology);
        let decoded = decode_payload(&source)?;
        Ok(Self {
            source,
            topology,
            identity,
            decoded: Arc::new(decoded),
        })
    }

    pub(super) fn browser_bytes(&self) -> PyResult<Vec<u8>> {
        self.source.encode_bcif()
    }
}

/// Encodes the producer's payload and decodes it back to the structure both
/// sides digest.
///
/// # Errors
///
/// Raises the producer's own exception, with its `MolFrame` code and message,
/// when the payload cannot be encoded, and a value error when the bytes the
/// producer wrote are not a structure.
fn decode_payload(source: &molframe_py::NativeStructureSource) -> PyResult<Structure> {
    payload_structure(&source.encode_bcif()?)
}

fn payload_structure(bytes: &[u8]) -> PyResult<Structure> {
    molgfx::source::structure_from_payload(bytes, "structure.bcif").ok_or_else(|| {
        PyValueError::new_err("the MolFrame BCIF payload is not a readable structure")
    })
}

fn decode_bond_order(code: u8, aromatic: bool) -> molgfx::molframe::BondOrder {
    match code {
        molframe_py::NativeBond::ORDER_SINGLE => molgfx::molframe::BondOrder::Single,
        molframe_py::NativeBond::ORDER_DOUBLE => molgfx::molframe::BondOrder::Double,
        molframe_py::NativeBond::ORDER_TRIPLE => molgfx::molframe::BondOrder::Triple,
        molframe_py::NativeBond::ORDER_QUADRUPLE => molgfx::molframe::BondOrder::Quadruple,
        molframe_py::NativeBond::ORDER_AROMATIC => molgfx::molframe::BondOrder::Aromatic,
        molframe_py::NativeBond::ORDER_POLYMERIC => molgfx::molframe::BondOrder::Polymeric,
        molframe_py::NativeBond::ORDER_UNKNOWN if aromatic => molgfx::molframe::BondOrder::Aromatic,
        _ => molgfx::molframe::BondOrder::Unknown,
    }
}

fn decode_secondary_structure(code: u8) -> PyResult<molgfx::molframe::SecondaryStructure> {
    use molgfx::molframe::SecondaryStructure;
    match code {
        0 => Ok(SecondaryStructure::Unknown),
        1 => Ok(SecondaryStructure::Coil),
        2 => Ok(SecondaryStructure::AlphaHelix),
        3 => Ok(SecondaryStructure::Strand),
        4 => Ok(SecondaryStructure::Turn),
        5 => Ok(SecondaryStructure::ThreeTenHelix),
        6 => Ok(SecondaryStructure::PiHelix),
        7 => Ok(SecondaryStructure::OtherHelix),
        8 => Ok(SecondaryStructure::BetaBridge),
        9 => Ok(SecondaryStructure::Bend),
        10 => Ok(SecondaryStructure::PolyProline),
        _ => Err(PyValueError::new_err(format!(
            "unknown secondary-structure code {code}"
        ))),
    }
}

#[cfg(test)]
#[path = "native_adapter_tests.rs"]
mod tests;

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
        // The capsule evaluates a query twice (count, then fill), at several
        // microseconds an atom, so the two queries whose answer is known from
        // the text alone never cross it.
        if let Some(constant) = constant_query(source, self.coordinates().len()) {
            return Ok(constant);
        }
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
    /// its own structure directly. Decoding the payload at import
    /// gives both sides the same molecule to digest, which is what makes the
    /// published content identity verifiable by its own consumer.
    fn molframe(&self) -> Option<&Structure> {
        Some(&self.decoded)
    }
}

/// The selection of a query that is `all` or `none`, however it is spelled.
///
/// `MolFrame` parses a lone `all`/`none` (case-insensitive, optionally
/// parenthesised) to the whole universe or the empty set before looking at any
/// atom, so the result is the same as evaluating it, in O(1).
fn constant_query(source: &str, atom_count: usize) -> Option<AtomSelection> {
    let mut text = source.trim();
    while let Some(inner) = text
        .strip_prefix('(')
        .and_then(|rest| rest.strip_suffix(')'))
    {
        text = inner.trim();
    }
    if text.eq_ignore_ascii_case("all") {
        Some(if atom_count == 0 {
            AtomSelection::Empty
        } else {
            AtomSelection::All
        })
    } else if text.eq_ignore_ascii_case("none") {
        Some(AtomSelection::Empty)
    } else {
        None
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

#[cfg(test)]
mod constant_query_tests {
    use super::*;

    #[test]
    fn lone_all_and_none_are_constants() {
        assert!(matches!(constant_query("all", 5), Some(AtomSelection::All)));
        assert!(matches!(
            constant_query(" ( ALL ) ", 5),
            Some(AtomSelection::All)
        ));
        assert!(matches!(
            constant_query("all", 0),
            Some(AtomSelection::Empty)
        ));
        assert!(matches!(
            constant_query("none", 5),
            Some(AtomSelection::Empty)
        ));
        assert!(constant_query("all and chain A", 5).is_none());
        assert!(constant_query("(all) or (none)", 5).is_none());
        assert!(constant_query("not all", 5).is_none());
    }
}
