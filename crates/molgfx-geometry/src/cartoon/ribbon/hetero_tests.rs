//! Heteroatoms are never backbone: a calcium ion is named `CA`, and so can a
//! ligand carbon, but neither is an alpha carbon of a polymer.

use super::*;
use crate::cartoon::ribbon::{RibbonParams, SplineProfile};
use molgfx_core::AtomSelection;
use std::fmt::Write as _;

/// A five-residue peptide, three Ca2+ ions (residue and atom `CA`) and a
/// three-residue non-polymer run whose carbon atoms are also named `CA`,
/// all spaced like a real backbone so that only chemistry separates them.
fn polymer_with_hetero_guides() -> molframe::Structure {
    let mut cif = String::from(
        "data_het\nloop_\n_entity.id\n_entity.type\n1 polymer\n2 non-polymer\n3 non-polymer\nloop_\n_atom_site.group_PDB\n_atom_site.id\n_atom_site.type_symbol\n_atom_site.label_atom_id\n_atom_site.label_comp_id\n_atom_site.label_asym_id\n_atom_site.label_entity_id\n_atom_site.label_seq_id\n_atom_site.Cartn_x\n_atom_site.Cartn_y\n_atom_site.Cartn_z\n",
    );
    let mut id = 1;
    for row in 0..5 {
        let x = f64::from(row) * 3.8;
        writeln!(cif, "ATOM {id} C CA ALA A 1 {} {x} 0 0", row + 1).expect("fixture");
        id += 1;
    }
    for row in 0..3 {
        let x = f64::from(row) * 3.8;
        writeln!(cif, "HETATM {id} CA CA CA B{row} 2 . {x} 10 0").expect("fixture");
        id += 1;
    }
    for row in 0..3 {
        let x = f64::from(row) * 3.8;
        writeln!(cif, "HETATM {id} C CA LIG C{row} 3 . {x} 20 0").expect("fixture");
        id += 1;
    }
    molframe::read_bytes(
        cif.into_bytes(),
        Some("het.cif"),
        &molframe::ReadOptions::new(),
    )
    .expect("fixture parses")
    .0
}

#[test]
fn ribbon_guides_come_only_from_polymer_residues() {
    let structure = polymer_with_hetero_guides();
    for profile in [
        SplineProfile::Cartoon,
        SplineProfile::NewRibbons,
        SplineProfile::Trace,
        SplineProfile::Tube,
    ] {
        let mut mesh = RibbonMesh::default();
        mesh.generate_structure(
            &structure,
            &AtomSelection::All,
            &[],
            8.0,
            RibbonParams {
                profile,
                ..RibbonParams::default()
            },
        )
        .expect("geometry");
        assert_eq!(
            mesh.traces.points.len(),
            5,
            "{profile:?}: only the peptide's five alpha carbons are guides"
        );
        assert!(
            mesh.traces
                .ranges()
                .iter()
                .all(|range| range.chain == 0 && range.points.end <= 5),
            "{profile:?}: no trace may start in a ligand or ion chain"
        );
    }
}
