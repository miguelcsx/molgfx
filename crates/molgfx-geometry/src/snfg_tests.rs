use super::*;
use molgfx_core::Scene;
use std::fmt::Write as _;

/// One complete six-atom ring per residue, laid out along x, with the given
/// CCD component ids. Rings are not linked: symbols depend on identity only.
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

fn hex(rgb: u32) -> Rgba8 {
    pack_rgb(rgb)
}

#[test]
fn spec_colours_and_shapes_follow_the_snfg_table() {
    let ids = ["NAG", "MAN", "BMA", "GAL", "A2G", "FUC", "SIA"];
    let structure = rings(&ids);
    let set = SnfgSymbolSet::build(&structure, &AtomSelection::All).expect("build");
    assert_eq!(set.len(), ids.len());
    let expected = [
        (CarbohydrateShape::Cube, 0x00_90BC),
        (CarbohydrateShape::Sphere, 0x00_A651),
        (CarbohydrateShape::Sphere, 0x00_A651),
        (CarbohydrateShape::Sphere, 0xFF_D400),
        (CarbohydrateShape::Cube, 0xFF_D400),
        (CarbohydrateShape::Triangle, 0xED_1C24),
        (CarbohydrateShape::Diamond, 0xA5_4399),
    ];
    for (index, (shape, color)) in expected.into_iter().enumerate() {
        assert_eq!(set.shapes[index], shape, "{}", ids[index]);
        assert_eq!(set.colors[index], hex(color), "{}", ids[index]);
    }
}

#[test]
fn poses_are_finite_unit_and_centred_on_the_ring() {
    let structure = rings(&["NAG", "NAG"]);
    let set = SnfgSymbolSet::build(&structure, &AtomSelection::All).expect("build");
    assert_eq!(set.len(), 2);
    assert!((set.centers[1].x - set.centers[0].x - 6.0).abs() < 1e-3);
    for orientation in &set.orientations {
        assert!((orientation.length_squared() - 1.0).abs() < 1e-4);
        // The rings lie in z = 0, so the symbol's thin axis is the z axis.
        assert!((*orientation * Vec3::Z).dot(Vec3::Z).abs() > 0.99);
    }
}

#[test]
fn non_saccharides_and_partial_selections_produce_nothing() {
    let structure = rings(&["NAG"]);
    let none = SnfgSymbolSet::build(&structure, &AtomSelection::Empty).expect("build");
    assert!(none.is_empty());
    assert!(none.to_primitives(handle_of(&structure), 1.0).is_empty());
    let partial = SnfgSymbolSet::build(&structure, &AtomSelection::Range(0..3)).expect("build");
    assert!(partial.is_empty());
}

fn handle_of(structure: &molframe::Structure) -> StructureHandle {
    Scene::from_structure(structure)
        .expect("scene")
        .structures()
        .next()
        .expect("structure")
        .0
}

#[test]
fn primitives_are_accepted_by_the_scene() {
    let structure = rings(&["NAG", "BMA", "FUC"]);
    let mut scene = Scene::from_structure(&structure).expect("scene");
    let owner = scene.structures().next().expect("structure").0;
    let set = SnfgSymbolSet::build(&structure, &AtomSelection::All).expect("build");
    let primitives = set.to_primitives(owner, 1.5);
    assert_eq!(primitives.len(), 3);
    scene.add_primitives(&primitives).expect("accepted");
    assert_eq!(scene.primitives().count(), 3);
    assert!(set.to_primitives(owner, 0.0).is_empty());
}

#[test]
fn every_upstream_shape_lowers_to_a_glyph_with_positive_size() {
    for glyph in [
        CarbohydrateShape::Sphere,
        CarbohydrateShape::Cube,
        CarbohydrateShape::Diamond,
        CarbohydrateShape::Triangle,
        CarbohydrateShape::Star,
        CarbohydrateShape::Bar,
        CarbohydrateShape::Hexagon,
        CarbohydrateShape::Pentagon,
    ] {
        let size = glyph_size(glyph);
        assert!(size.min_element() > 0.0);
    }
}

#[test]
fn subset_keeps_only_fully_selected_rings() {
    let structure = rings(&["NAG", "MAN", "FUC"]);
    let all = SnfgSymbolSet::build(&structure, &AtomSelection::All).expect("build");
    assert_eq!(all.len(), 3);
    assert_eq!(all.subset(&AtomSelection::All), all);
    assert!(all.subset(&AtomSelection::Empty).is_empty());
    // Each ring has six atoms, so rows 6..12 are exactly the second residue.
    let middle = all.subset(&AtomSelection::Range(6..12));
    assert_eq!(middle.len(), 1);
    assert_eq!(middle.shapes[0], all.shapes[1]);
    assert_eq!(middle.centers[0], all.centers[1]);
    let direct = SnfgSymbolSet::build(&structure, &AtomSelection::Range(6..12)).expect("build");
    assert_eq!(middle, direct);
    assert!(all.subset(&AtomSelection::Range(6..11)).is_empty());
}
