//! Structure size classes.

/// Polymer residue counts below which a structure is still small or medium.
const SMALL_RESIDUES: u64 = 10;
const MEDIUM_RESIDUES: u64 = 5_000;
const LARGE_RESIDUES: u64 = 30_000;

/// Atom count above which even an atomic-detail structure is drawn as lines.
const LINE_ATOMS: u64 = 100_000;
/// Atom count above which an atomic-detail structure is drawn as points.
const POINT_ATOMS: u64 = 1_000_000;

/// How large a structure is, by the number of polymer residues it holds.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum StructureSize {
    /// Fewer than ten polymer residues: a ligand, peptide or small molecule.
    Small,
    /// Up to five thousand polymer residues: a typical protein or complex.
    Medium,
    /// Up to thirty thousand polymer residues: a large assembly.
    Large,
    /// Beyond that: a viral capsid, ribosome collection or cell fragment.
    Huge,
}

impl StructureSize {
    /// Classifies a structure from its polymer residue count.
    #[must_use]
    pub const fn from_polymer_residues(residues: u64) -> Self {
        if residues < SMALL_RESIDUES {
            Self::Small
        } else if residues < MEDIUM_RESIDUES {
            Self::Medium
        } else if residues < LARGE_RESIDUES {
            Self::Large
        } else {
            Self::Huge
        }
    }
}

/// Atomic-detail form for a structure of `atoms` atoms.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum AtomicDetail {
    BallAndStick,
    Lines,
    Points,
}

impl AtomicDetail {
    pub(super) const fn for_atoms(atoms: u64) -> Self {
        if atoms >= POINT_ATOMS {
            Self::Points
        } else if atoms >= LINE_ATOMS {
            Self::Lines
        } else {
            Self::BallAndStick
        }
    }
}
