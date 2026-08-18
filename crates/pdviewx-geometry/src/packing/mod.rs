//! Scene-to-GPU instance packing.

pub(crate) mod pack;

pub use pack::{
    PropertyColumns, RibbonColoring, build_compaction_map, pack_atoms, pack_atoms_with_hierarchy,
    pack_atoms_with_properties, pack_bonds, pack_residue_beads, recolor_ribbon,
    recolor_ribbon_with_appearance,
};
