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
    /// Chemical bond order from the source model.
    pub order: molframe::BondOrder,
    /// Whether the source assigns aromatic order.
    pub aromatic: bool,
    /// Whether the edge is a metal coordination bond.
    pub metal: bool,
}

impl SourceBond {
    /// Quantizes source chemistry to the styles supported by the analytic bond shader.
    #[must_use]
    pub const fn gpu_order(self) -> u32 {
        match self.order {
            molframe::BondOrder::Double | molframe::BondOrder::Aromatic => 2,
            molframe::BondOrder::Triple | molframe::BondOrder::Quadruple => 3,
            molframe::BondOrder::Single
            | molframe::BondOrder::Polymeric
            | molframe::BondOrder::Unknown => 1,
        }
    }
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
    /// Anisotropic displacement tensors, one row per atom that carries one.
    ///
    /// Sparse by design: most structures are isotropic, so an absent entry
    /// means the atom has no recorded ellipsoid. Rows ascend by atom and each
    /// tensor is `[U11, U22, U33, U12, U13, U23]` in ångström squared, the same
    /// order the renderer's ellipsoid primitive consumes.
    pub anisotropy: Arc<[(u32, [f32; 6])]>,
    /// File or analysis secondary structure aligned to residue rows.
    pub secondary_structure: Arc<[molframe::SecondaryStructure]>,
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

    /// Evaluates an already-compiled query.
    ///
    /// Providers that retain a native `MolFrame` structure override this so a
    /// query compiled once is not recompiled per placed structure. The default
    /// path is correct for providers that only expose textual evaluation.
    ///
    /// # Errors
    ///
    /// Returns an error when the query cannot be evaluated.
    fn select_compiled(&self, query: &molframe::Query) -> Result<AtomSelection, CoreError> {
        self.select(query.source())
    }

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

    /// Evaluation of an already-compiled query.
    ///
    /// # Errors
    ///
    /// Returns an error when the query cannot be evaluated.
    pub fn select_compiled(&self, query: &molframe::Query) -> Result<AtomSelection, CoreError> {
        self.0.select_compiled(query)
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
        let query = molframe::Query::compile(source).map_err(|_| CoreError::InvalidSelection {
            reason: "MolFrame query evaluation failed",
        })?;
        self.select_compiled_query(&query)
    }

    fn select_compiled(&self, query: &molframe::Query) -> Result<AtomSelection, CoreError> {
        self.select_compiled_query(query)
    }

    fn molframe(&self) -> Option<&molframe::Structure> {
        Some(&self.structure)
    }
}

impl MolframeProvider {
    fn select_compiled_query(&self, query: &molframe::Query) -> Result<AtomSelection, CoreError> {
        let selection = molframe::QueryStructure::select_query(
            &self.structure,
            query,
            &molframe::AnalysisPolicy::default(),
        )
        .map_err(|_| CoreError::InvalidSelection {
            reason: "MolFrame query evaluation failed",
        })?;
        let selected = selection.selection.len();
        Ok(adaptive(selected, self.structure.atom_count(), || {
            selection.selection.iter().collect::<RoaringBitmap>()
        }))
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
        .map(|bond| {
            let metal = structure
                .engine()
                .atom(bond.atom_a)
                .and_then(molframe::AtomRef::element)
                .zip(
                    structure
                        .engine()
                        .atom(bond.atom_b)
                        .and_then(molframe::AtomRef::element),
                )
                .is_some_and(|(left, right)| is_metal(left) || is_metal(right));
            SourceBond {
                atoms: [bond.atom_a.get(), bond.atom_b.get()],
                order: bond.order,
                aromatic: bond.order == molframe::BondOrder::Aromatic,
                metal,
            }
        })
        .collect::<Vec<_>>();
    SourceTopology {
        atoms: atoms.into(),
        residue_atom_start: residue_atom_start.into(),
        chain_residue_start: chain_residue_start.into(),
        model_chain_start: model_chain_start.into(),
        bonds: bonds.into(),
        anisotropy: anisotropy(structure).into(),
        secondary_structure: structure.engine().secondary_structure().to_vec().into(),
    }
}

/// Collects the anisotropic displacement tensors that ascend by atom row.
///
/// The facade exposes the table through `Structure::anisotropy`, whose rows are
/// already sorted by atom, so this is one pass over the populated entries.
/// A structure without the category yields an empty column.
fn anisotropy(structure: &molframe::Structure) -> Vec<(u32, [f32; 6])> {
    let table = structure.anisotropy();
    if !table.is_available() {
        return Vec::new();
    }
    table
        .iter()
        .map(|ellipsoid| (ellipsoid.atom.get(), ellipsoid.u))
        .collect()
}

fn is_metal(element: molframe::Element) -> bool {
    is_metal_atomic_number(element.atomic_number())
}

/// Returns whether an atomic number is rendered as a metal-coordination endpoint.
#[must_use]
pub const fn is_metal_atomic_number(atomic_number: u8) -> bool {
    matches!(
        atomic_number,
        3   // Li
            | 11 // Na
            | 19 // K
            | 12 // Mg
            | 20 // Ca
            | 25 // Mn
            | 26 // Fe
            | 27 // Co
            | 28 // Ni
            | 29 // Cu
            | 30 // Zn
            | 42 // Mo
            | 47 // Ag
            | 48 // Cd
            | 80 // Hg
            | 79 // Au
            | 13 // Al
            | 14 // Si
            | 24 // Cr
            | 23 // V
            | 74 // W
    )
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

/// Narrows a `MolFrame` selection to the densest shape that holds it.
///
/// The membership is asked for once as a count, so a whole-structure or empty
/// result is answered without touching a single atom. Only a partial result is
/// enumerated into a bitmap, which is the one case where the bitmap is the
/// representation that is kept rather than a temporary.
fn adaptive(selected: u64, table_len: u32, rows: impl FnOnce() -> RoaringBitmap) -> AtomSelection {
    if selected == 0 {
        return AtomSelection::Empty;
    }
    if selected == u64::from(table_len) {
        return AtomSelection::All;
    }
    AtomSelection::Roaring(rows())
}

#[cfg(test)]
#[path = "source_tests.rs"]
mod tests;
