//! Scene-to-GPU instance packing.

mod bonds;
mod error;
mod overlay;
pub(crate) mod pack;

pub use bonds::{build_compaction_map, build_selection_compaction, pack_bonds};
pub use error::PackingError;
pub use overlay::OverlayColumn;
pub use pack::{
    PropertyColumns, RibbonColoring, pack_atoms, pack_atoms_with_hierarchy,
    pack_atoms_with_properties, pack_residue_beads, recolor_ribbon, recolor_ribbon_with_appearance,
};
