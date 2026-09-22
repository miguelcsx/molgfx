//! Structures generated in memory for benchmarks that must always run.
//!
//! A benchmark that silently skips when a corpus is absent measures nothing and
//! still reports success, so the semantic-layer benchmarks build their own
//! input. The generated structure is a plain alanine chain: uninteresting
//! chemically, but it exercises the real parser, the real topology columns and
//! the real coordinate seam at whatever size a measurement needs.

use std::fmt::Write as _;

// Backbone plus two side-chain atoms, laid out along a gentle helix so the
// coordinates are distinct and spatial queries have something to do. The
// per-atom offset is tabulated rather than derived from the loop index, so
// no integer ever has to be converted to a float.
const ATOMS: [(&str, &str, f32); 5] = [
    ("N", "N", 0.0),
    ("C", "CA", 1.0),
    ("C", "C", 2.0),
    ("O", "O", 3.0),
    ("C", "CB", 4.0),
];

/// Builds an mmCIF document with `residues` alanine residues on one chain.
#[must_use]
pub fn alanine_chain(residues: usize) -> Vec<u8> {
    let mut text = String::with_capacity(residues * 5 * 96 + 512);
    text.push_str(
        "data_synthetic\n_entry.id synthetic\nloop_\n_atom_site.group_PDB\n_atom_site.id\n\
         _atom_site.type_symbol\n_atom_site.label_atom_id\n_atom_site.label_comp_id\n\
         _atom_site.label_asym_id\n_atom_site.label_entity_id\n_atom_site.label_seq_id\n\
         _atom_site.Cartn_x\n_atom_site.Cartn_y\n_atom_site.Cartn_z\n_atom_site.occupancy\n\
         _atom_site.B_iso_or_equiv\n",
    );
    let mut serial = 1_u32;
    let mut turn = 0.0_f32;
    for residue in 0..residues {
        for (element, name, offset) in ATOMS {
            let x = turn.cos().mul_add(4.0, offset * 0.4);
            let y = turn.sin().mul_add(4.0, offset * 0.4);
            let z = turn.mul_add(1.5, offset * 0.3);
            let _ = writeln!(
                text,
                "ATOM {serial} {element} {name} ALA A 1 {} {x:.3} {y:.3} {z:.3} 1.00 20.00",
                residue + 1
            );
            serial += 1;
        }
        turn += 0.6;
    }
    text.into_bytes()
}

/// Parses a generated chain, panicking only in a benchmark build.
///
/// # Panics
///
/// Panics when the generated document does not parse, which would mean this
/// module emits malformed mmCIF rather than that a measurement failed.
#[must_use]
pub fn structure(residues: usize) -> molframe::Structure {
    let bytes = alanine_chain(residues);
    match molframe::read_bytes(bytes, Some("synthetic.cif"), &molframe::ReadOptions::new()) {
        Ok((structure, _)) => structure,
        Err(error) => panic!("generated mmCIF must parse: {error:?}"),
    }
}
