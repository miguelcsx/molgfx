use crate::representation::RepresentationRequirement;
use crate::{Scene, rep};
use std::fmt::Write as _;

fn rings(components: &[&str]) -> molframe::Structure {
    let mut cif = String::from(
        "data_snfg\nloop_\n_atom_site.group_PDB\n_atom_site.id\n_atom_site.type_symbol\n_atom_site.label_atom_id\n_atom_site.label_alt_id\n_atom_site.label_comp_id\n_atom_site.label_asym_id\n_atom_site.label_entity_id\n_atom_site.label_seq_id\n_atom_site.Cartn_x\n_atom_site.Cartn_y\n_atom_site.Cartn_z\n_atom_site.occupancy\n_atom_site.B_iso_or_equiv\n_atom_site.auth_seq_id\n_atom_site.auth_asym_id\n_atom_site.pdbx_PDB_model_num\n",
    );
    let names = ["C1", "C2", "C3", "C4", "C5", "O5"];
    let mut id = 1;
    for (residue, component) in components.iter().enumerate() {
        let seq = residue + 1;
        let x = f32::from(u16::try_from(residue).expect("few residues")) * 6.0;
        for (corner, name) in names.iter().enumerate() {
            let angle =
                f32::from(u8::try_from(corner).expect("six corners")) * std::f32::consts::TAU / 6.0;
            let element = if name.starts_with('O') { "O" } else { "C" };
            writeln!(
                cif,
                "HETATM {id} {element} {name} . {component} A 1 {seq} {} {} 0.0 1.00 10.0 {seq} A 1",
                x + angle.cos() * 1.4,
                angle.sin() * 1.4
            )
            .unwrap();
            id += 1;
        }
    }
    cif.push_str("#\nloop_\n_struct_conn.id\n_struct_conn.conn_type_id\n_struct_conn.ptnr1_label_asym_id\n_struct_conn.ptnr1_label_seq_id\n_struct_conn.ptnr1_label_comp_id\n_struct_conn.ptnr1_label_atom_id\n_struct_conn.ptnr2_label_asym_id\n_struct_conn.ptnr2_label_seq_id\n_struct_conn.ptnr2_label_comp_id\n_struct_conn.ptnr2_label_atom_id\n");
    let mut edge = 0;
    for (residue, component) in components.iter().enumerate() {
        for corner in 0..6 {
            writeln!(
                cif,
                "b{edge} covale A {} {component} {} A {} {component} {}",
                residue + 1,
                names[corner],
                residue + 1,
                names[(corner + 1) % 6]
            )
            .unwrap();
            edge += 1;
        }
    }
    molframe::read_bytes(
        cif.into_bytes(),
        Some("snfg.cif"),
        &molframe::ReadOptions::new(),
    )
    .expect("source")
    .0
}

fn glycan() -> molframe::Structure {
    rings(&["NAG", "NAG", "MAN", "FUC"])
}

#[test]
fn snfg_emits_one_primitive_per_monosaccharide_and_clears_on_hide() {
    let mut scene = Scene::from_structure(&glycan()).unwrap();
    let id = scene.add(rep::snfg("all")).unwrap();
    assert_eq!(scene.overlay_handles().snfg_symbols, 4);
    assert_eq!(scene.resolved().primitives().count(), 4);
    scene.set_visible(id, false).unwrap();
    assert_eq!(scene.overlay_handles().snfg_symbols, 0);
    assert_eq!(scene.resolved().primitives().count(), 0);
    scene.set_visible(id, true).unwrap();
    assert_eq!(scene.resolved().primitives().count(), 4);
    let patch = scene.transaction(|edit| edit.remove(id)).unwrap();
    assert_eq!(patch.operations.len(), 1);
    assert_eq!(scene.resolved().primitives().count(), 0);
}

#[test]
fn snfg_respects_the_selection_and_scale() {
    let mut scene = Scene::from_structure(&glycan()).unwrap();
    scene
        .add(rep::snfg("resid 3 or resid 4").scale(2.0))
        .unwrap();
    assert_eq!(scene.resolved().primitives().count(), 2);
    assert!(scene.add(rep::snfg("all").scale(0.0)).is_err());
}

#[test]
fn snfg_needs_monosaccharides() {
    let scene = Scene::from_structure(&super::tests::structure()).unwrap();
    let applicability = scene
        .representation_applicability(rep::snfg("all"))
        .unwrap();
    assert_eq!(
        applicability.missing,
        Some(RepresentationRequirement::Monosaccharides)
    );
    let scene = Scene::from_structure(&glycan()).unwrap();
    assert!(
        scene
            .representation_applicability(rep::snfg("all"))
            .unwrap()
            .is_applicable()
    );
}

#[test]
fn symbol_sets_are_cached_per_structure_revision() {
    let structure = glycan();
    let mut scene = Scene::from_structure(&structure).unwrap();
    scene.add(rep::snfg("all")).unwrap();
    let source = molgfx_core::MolecularSource::from_molframe(&structure);
    let first = super::snfg_symbols::symbols_of(&source).unwrap();
    let second = super::snfg_symbols::symbols_of(&source).unwrap();
    assert!(std::sync::Arc::ptr_eq(&first, &second));
    assert!(super::snfg_symbols::cached_len() >= 1);
}
