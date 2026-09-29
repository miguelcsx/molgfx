//! Scene-to-GPU instance packing.

mod bonds;
mod color;
mod error;
pub(crate) mod pack;

pub use bonds::{build_compaction_map, build_selection_compaction, pack_bonds};
pub use color::ColorContext;
pub use error::PackingError;
pub use pack::{RibbonColoring, pack_atoms, pack_residue_beads, recolor_ribbon};
