use super::*;
use crate::RibbonParams;
use std::fmt::Write;

fn sugar() -> molframe::Structure {
    let cif = "data_sugar\nloop_\n_atom_site.group_PDB\n_atom_site.id\n_atom_site.type_symbol\n_atom_site.label_atom_id\n_atom_site.label_comp_id\n_atom_site.label_asym_id\n_atom_site.label_entity_id\n_atom_site.label_seq_id\n_atom_site.Cartn_x\n_atom_site.Cartn_y\n_atom_site.Cartn_z\nATOM 1 O \"O4'\" DA A 1 1 0 1.5 0\nATOM 2 C \"C1'\" DA A 1 1 1.4 0.5 0\nATOM 3 C \"C2'\" DA A 1 1 0.9 -1.2 0\nATOM 4 C \"C3'\" DA A 1 1 -0.9 -1.2 0\nATOM 5 C \"C4'\" DA A 1 1 -1.4 0.5 0\n";
    molframe::read_bytes(
        cif.as_bytes().to_vec(),
        Some("sugar.cif"),
        &molframe::ReadOptions::new(),
    )
    .unwrap()
    .0
}

#[test]
fn a_single_sugar_has_exact_atom_triangles_without_a_backbone_trace() {
    let source = sugar();
    let mut mesh = RibbonMesh::default();
    mesh.generate_legacy_ribbons(
        &source,
        &AtomSelection::All,
        8.0,
        RibbonParams::default(),
        0.15,
    )
    .unwrap();
    assert_eq!(mesh.vertices.len(), 9);
    assert_eq!(mesh.indices.len(), 9);
    assert!(has_selected_sugar_faces(&source, &AtomSelection::All));
    assert!(
        mesh.deformations
            .iter()
            .all(|recipe| recipe.parameter[3].to_bits() == 10)
    );
    for (index, (vertex, recipe)) in mesh.vertices.iter().zip(&mesh.deformations).enumerate() {
        assert_eq!(
            recipe.parameter[0].to_bits(),
            f32::from([0u8, 1, 2][index % 3]).to_bits()
        );
        let row = recipe.controls[index % 3];
        let position = source
            .engine()
            .atom(molframe::AtomIndex::new(row))
            .unwrap()
            .position()
            .unwrap();
        assert_eq!(
            vertex.position.map(f32::to_bits),
            position.map(f32::to_bits)
        );
    }
}

#[test]
fn an_incomplete_sugar_selection_emits_no_invented_face() {
    let source = sugar();
    let selection = AtomSelection::Range(0..4);
    let mut mesh = RibbonMesh::default();
    mesh.generate_legacy_ribbons(&source, &selection, 8.0, RibbonParams::default(), 0.15)
        .unwrap();
    assert!(mesh.vertices.is_empty());
    assert!(!has_selected_sugar_faces(&source, &selection));
}

#[test]
fn both_ribbon_methods_include_nucleotide_bases_and_keep_live_atom_anchors() {
    let sugar = sugar();
    let atoms = [
        ("N1", "N", [3.0, 0.0, 0.0]),
        ("C2", "C", [4.0, 0.0, 0.0]),
        ("N3", "N", [4.5, 1.0, 0.0]),
        ("C4", "C", [4.0, 2.0, 0.0]),
        ("C5", "C", [3.0, 2.0, 0.0]),
        ("C6", "C", [2.5, 1.0, 0.0]),
        ("N7", "N", [3.0, 3.0, 0.0]),
        ("C8", "C", [4.0, 3.5, 0.0]),
        ("N9", "N", [4.5, 2.5, 0.0]),
    ];
    let mut cif = String::from(
        "data_base\nloop_\n_atom_site.group_PDB\n_atom_site.id\n_atom_site.type_symbol\n_atom_site.label_atom_id\n_atom_site.label_comp_id\n_atom_site.label_asym_id\n_atom_site.label_entity_id\n_atom_site.label_seq_id\n_atom_site.Cartn_x\n_atom_site.Cartn_y\n_atom_site.Cartn_z\n",
    );
    for atom in sugar.atoms() {
        let [x, y, z] = atom.position().unwrap();
        writeln!(
            cif,
            "ATOM {} {} \"{}\" DA A 1 1 {x} {y} {z}",
            atom.index().get() + 1,
            atom.element().unwrap(),
            atom.name().unwrap()
        )
        .unwrap();
    }
    for (i, (name, element, [x, y, z])) in atoms.into_iter().enumerate() {
        writeln!(cif, "ATOM {} {element} {name} DA A 1 1 {x} {y} {z}", i + 6).unwrap();
    }
    let source = molframe::read_bytes(
        cif.into_bytes(),
        Some("base.cif"),
        &molframe::ReadOptions::new(),
    )
    .unwrap()
    .0;
    for newer in [false, true] {
        let mut mesh = RibbonMesh::default();
        if newer {
            mesh.generate_structure(
                &source,
                &AtomSelection::All,
                &[],
                8.0,
                RibbonParams {
                    profile: super::super::SplineProfile::NewRibbons,
                    ..RibbonParams::default()
                },
            )
            .unwrap();
        } else {
            mesh.generate_legacy_ribbons(
                &source,
                &AtomSelection::All,
                8.0,
                RibbonParams::default(),
                0.15,
            )
            .unwrap();
        }
        assert_eq!(mesh.vertices.len(), 30);
        for (index, (vertex, recipe)) in mesh.vertices.iter().zip(&mesh.deformations).enumerate() {
            let row = recipe.controls[index % 3];
            assert_eq!(
                vertex.position.map(f32::to_bits),
                source
                    .engine()
                    .atom(molframe::AtomIndex::new(row))
                    .unwrap()
                    .position()
                    .unwrap()
                    .map(f32::to_bits)
            );
        }
    }
}

#[test]
fn a_shared_ring_atom_does_not_join_incompatible_alternate_conformers() {
    let cif = "data_alternate\nloop_\n_atom_site.group_PDB\n_atom_site.id\n_atom_site.type_symbol\n_atom_site.label_atom_id\n_atom_site.label_alt_id\n_atom_site.label_comp_id\n_atom_site.label_asym_id\n_atom_site.label_entity_id\n_atom_site.label_seq_id\n_atom_site.Cartn_x\n_atom_site.Cartn_y\n_atom_site.Cartn_z\nATOM 1 O \"O4'\" . DA A 1 1 0 1.5 0\nATOM 2 C \"C1'\" A DA A 1 1 1.4 0.5 0\nATOM 3 C \"C2'\" B DA A 1 1 0.9 -1.2 0\nATOM 4 C \"C3'\" A DA A 1 1 -0.9 -1.2 0\nATOM 5 C \"C4'\" A DA A 1 1 -1.4 0.5 0\n";
    let source = molframe::read_bytes(
        cif.as_bytes().to_vec(),
        Some("alternate.cif"),
        &molframe::ReadOptions::new(),
    )
    .unwrap()
    .0;
    assert!(!has_selected_sugar_faces(&source, &AtomSelection::All));
    let mut mesh = RibbonMesh::default();
    mesh.generate_legacy_ribbons(
        &source,
        &AtomSelection::All,
        8.0,
        RibbonParams::default(),
        0.15,
    )
    .unwrap();
    assert!(mesh.vertices.is_empty());
}
