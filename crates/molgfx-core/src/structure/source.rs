//! Provider-neutral immutable molecular storage retained by scene assets.

use crate::{AtomSelection, CoreError};
use roaring::RoaringBitmap;
use std::fmt;
use std::sync::Arc;

/// Compact atom metadata materialized once for renderer-side dense access.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SourceAtom {
    /// Atomic number, or zero when unknown.
    pub element: u8,
    /// Owning residue row.
    pub residue: u32,
}

/// Compact covalent-bond metadata used by renderer packing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SourceBond {
    /// Endpoint atom rows.
    pub atoms: [u32; 2],
    /// Whether the source assigns aromatic order.
    pub aromatic: bool,
}

/// Compact hierarchy and connectivity associated with one coordinate column.
#[derive(Clone, Debug, Default)]
pub struct SourceTopology {
    /// Atoms in coordinate order.
    pub atoms: Arc<[SourceAtom]>,
    /// Residue-to-atom start offsets including the final end offset.
    pub residue_atom_start: Arc<[u32]>,
    /// Chain-to-residue start offsets including the final end offset.
    pub chain_residue_start: Arc<[u32]>,
    /// Model-to-chain start offsets including the final end offset.
    pub model_chain_start: Arc<[u32]>,
    /// Covalent-bond records.
    pub bonds: Arc<[SourceBond]>,
}

/// Borrowed molecular source contract used by physical structure assets.
pub trait MolecularProvider: fmt::Debug + Send + Sync {
    /// Stable identity of this immutable asset.
    fn identity(&self) -> u64;

    /// Parser- or provider-owned coordinates in atom order.
    fn coordinates(&self) -> &[[f32; 3]];

    /// Revision of the coordinate column.
    fn coordinate_revision(&self) -> u64;

    /// Compact topology aligned with `coordinates`.
    fn topology(&self) -> &SourceTopology;

    /// Evaluates the canonical `MolFrame` selection language.
    ///
    /// # Errors
    ///
    /// Returns an error when the query is invalid or cannot be evaluated.
    fn select(&self, source: &str) -> Result<AtomSelection, CoreError>;

    /// Native `MolFrame` source when this provider originated in Rust.
    fn molframe(&self) -> Option<&molframe::Structure> {
        None
    }
}

/// Cheap shared handle over one provider-neutral molecular source.
#[derive(Clone)]
pub struct MolecularSource(Arc<dyn MolecularProvider>);

impl fmt::Debug for MolecularSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MolecularSource")
            .field("identity", &self.identity())
            .field("atoms", &self.coordinates().len())
            .finish_non_exhaustive()
    }
}

impl MolecularSource {
    /// Retains an arbitrary provider implementation.
    #[must_use]
    pub fn new(provider: impl MolecularProvider + 'static) -> Self {
        Self(Arc::new(provider))
    }

    /// Adapts a `MolFrame` snapshot without copying its coordinate column.
    #[must_use]
    pub fn from_molframe(structure: &molframe::Structure) -> Self {
        Self::new(MolframeProvider::new(structure))
    }

    /// Stable asset identity.
    #[must_use]
    pub fn identity(&self) -> u64 {
        self.0.identity()
    }

    /// Borrowed coordinate column.
    #[must_use]
    pub fn coordinates(&self) -> &[[f32; 3]] {
        self.0.coordinates()
    }

    /// Coordinate revision.
    #[must_use]
    pub fn coordinate_revision(&self) -> u64 {
        self.0.coordinate_revision()
    }

    /// Compact topology.
    #[must_use]
    pub fn topology(&self) -> &SourceTopology {
        self.0.topology()
    }

    /// Canonical query evaluation.
    ///
    /// # Errors
    ///
    /// Returns an error when the query is invalid or cannot be evaluated.
    pub fn select(&self, source: &str) -> Result<AtomSelection, CoreError> {
        self.0.select(source)
    }

    /// Native source when available.
    #[must_use]
    pub fn molframe(&self) -> Option<&molframe::Structure> {
        self.0.molframe()
    }

    /// Whether two handles retain the same provider instance.
    #[must_use]
    pub fn shares_storage_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

#[derive(Debug)]
struct MolframeProvider {
    structure: molframe::Structure,
    identity: u64,
    topology: SourceTopology,
}

impl MolframeProvider {
    fn new(structure: &molframe::Structure) -> Self {
        Self {
            structure: structure.clone(),
            identity: structure.coordinates().as_ptr() as usize as u64,
            topology: topology(structure),
        }
    }
}

impl MolecularProvider for MolframeProvider {
    fn identity(&self) -> u64 {
        self.identity
    }

    fn coordinates(&self) -> &[[f32; 3]] {
        self.structure.coordinates()
    }

    fn coordinate_revision(&self) -> u64 {
        self.structure.engine().generation().get()
    }

    fn topology(&self) -> &SourceTopology {
        &self.topology
    }

    fn select(&self, source: &str) -> Result<AtomSelection, CoreError> {
        let selection = self
            .structure
            .select(source, &molframe::AnalysisPolicy::default())
            .map_err(|_| CoreError::InvalidSelection {
                reason: "MolFrame query evaluation failed",
            })?;
        let rows = selection
            .atoms()
            .map(|atom| atom.index().get())
            .collect::<RoaringBitmap>();
        Ok(adaptive(rows, self.structure.atom_count()))
    }

    fn molframe(&self) -> Option<&molframe::Structure> {
        Some(&self.structure)
    }
}

fn topology(structure: &molframe::Structure) -> SourceTopology {
    let data = structure.engine().data();
    let atoms = data
        .atoms()
        .map(|atom| SourceAtom {
            element: atom.element().map_or(0, molframe::Element::atomic_number),
            residue: atom.residue().map_or(0, |residue| residue.index().get()),
        })
        .collect::<Vec<_>>();
    let residue_atom_start = offsets(
        data.residues()
            .map(|residue| residue.atoms().map(|atom| atom.index().get())),
    );
    let chain_residue_start = offsets(
        data.chains()
            .map(|chain| chain.residues().map(|residue| residue.index().get())),
    );
    let model_chain_start = offsets(
        data.models()
            .map(|model| model.chains().map(|chain| chain.index().get())),
    );
    let bonds = structure
        .bonds()
        .iter()
        .map(|bond| SourceBond {
            atoms: [bond.atom_a.get(), bond.atom_b.get()],
            aromatic: bond.order == molframe::BondOrder::Aromatic,
        })
        .collect::<Vec<_>>();
    SourceTopology {
        atoms: atoms.into(),
        residue_atom_start: residue_atom_start.into(),
        chain_residue_start: chain_residue_start.into(),
        model_chain_start: model_chain_start.into(),
        bonds: bonds.into(),
    }
}

fn offsets<I, R>(rows: I) -> Vec<u32>
where
    I: Iterator<Item = R>,
    R: Iterator<Item = u32>,
{
    let mut starts = Vec::new();
    let mut end = 0;
    for row in rows {
        let mut row = row.peekable();
        let mut start = end;
        if let Some(value) = row.peek().copied() {
            start = value;
        }
        starts.push(start);
        end = row.last().map_or(start, |value| value.saturating_add(1));
    }
    starts.push(end);
    starts
}

fn adaptive(rows: RoaringBitmap, table_len: u32) -> AtomSelection {
    if rows.is_empty() {
        AtomSelection::Empty
    } else if rows.len() == u64::from(table_len) {
        AtomSelection::All
    } else {
        AtomSelection::Roaring(rows)
    }
}
