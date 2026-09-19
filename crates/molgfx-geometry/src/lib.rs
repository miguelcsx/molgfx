//! GPU geometry generation: impostor packing, cartoons, labels, bounding
//! hierarchies.
//!
//! An atom or bond impostor needs no generation beyond packing its instance
//! record; the primitive's shape is reconstructed per pixel in the fragment
//! shader, never tessellated here.

#![forbid(unsafe_code)]

mod cartoon;
mod packing;
mod polyhedra;

pub use cartoon::{
    PolymerTraces, RibbonMesh, RibbonParams, RibbonVertex, SecondaryMotion, SplineProfile,
    TraceRange, append_base_polygons, append_base_slabs, append_paper_chain,
    extract_glycosidic_traces, extract_polymer_traces, solve_offsets, variable_tube_radius,
};
pub use packing::{
    PackingError, PropertyColumns, RibbonColoring, build_compaction_map,
    build_selection_compaction, pack_atoms, pack_atoms_with_hierarchy, pack_atoms_with_properties,
    pack_bonds, pack_residue_beads, recolor_ribbon, recolor_ribbon_with_appearance,
};
pub use polyhedra::{MAX_SHELL, coordination_hull};
