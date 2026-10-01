//! Size-aware default representations.
//!
//! A structure of ten residues and one of ten thousand should not be drawn the
//! same way: atomic detail is the right default for the first and an
//! unreadable, expensive one for the second. [`StructureSize`] classifies a
//! structure once from its polymer residue count and [`auto_representations`]
//! turns the class into a small, fixed set of forms.
//!
//! Cost is one selection evaluation per chemical class plus one pass over the
//! polymer atoms to count residues, `O(atoms)`, and nothing per frame.

mod auto;
mod ensemble;
mod pocket;
mod size;

#[cfg(test)]
mod tests;

pub use auto::auto_representations;
pub use ensemble::{
    DifferenceStyle, EnsembleMember, EnsembleStyle, difference_visual, ensemble_representations,
};
pub use pocket::{PocketStyle, pocket_representations};
pub use size::StructureSize;
