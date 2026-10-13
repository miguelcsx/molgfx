use super::tests::structure;
use super::*;
use crate::id::StructureId;
use crate::spec::SceneSpec;
use crate::{rep, sel};

const SHIFT_X: [f32; 16] = [
    1.0, 0.0, 0.0, 0.0, //
    0.0, 1.0, 0.0, 0.0, //
    0.0, 0.0, 1.0, 0.0, //
    25.0, 0.0, 0.0, 1.0,
];

fn scene() -> Scene {
    Scene::from_structure(&structure()).unwrap_or_else(|error| panic!("{error}"))
}

#[test]
fn a_placed_copy_is_its_own_structure_reading_the_same_atoms_elsewhere() {
    let mut scene = scene();
    let original = StructureId(1);
    let copy = scene
        .place(original, SHIFT_X)
        .unwrap_or_else(|error| panic!("{error}"));
    assert_ne!(copy, original);
    let spec = scene.spec();
    assert_eq!(
        spec.structures[&original].content_hash,
        spec.structures[&copy].content_hash
    );
    assert_eq!(spec.structures[&copy].placement, Some(SHIFT_X));
    assert_eq!(spec.structures[&original].placement, None);

    let handles: Vec<_> = scene.resolved().structures().collect();
    assert_eq!(handles.len(), 2);
    let moved = handles[1].1.model_to_world.w_axis.x;
    assert!((moved - 25.0).abs() < 1e-6, "{moved}");
    assert!(handles[0].1.model_to_world.w_axis.x.abs() < 1e-6);
    scene
        .add(rep::spacefill(sel::all()).structure(copy))
        .unwrap_or_else(|error| panic!("{error}"));
}

#[test]
fn a_placement_survives_the_wire_form_and_unplaced_specs_are_unchanged() {
    let mut scene = scene();
    let plain = scene
        .to_spec()
        .to_json()
        .unwrap_or_else(|error| panic!("{error}"));
    assert!(!plain.contains("placement"));
    let _ = scene
        .place(StructureId(1), SHIFT_X)
        .unwrap_or_else(|error| panic!("{error}"));
    let json = scene
        .to_spec()
        .to_json()
        .unwrap_or_else(|error| panic!("{error}"));
    assert!(json.contains("placement"));
    let again = SceneSpec::from_json(&json).unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(again, scene.to_spec());
}

#[test]
fn bad_placements_and_unknown_structures_change_nothing() {
    let mut scene = scene();
    let before = scene.to_spec();
    let mut nan = SHIFT_X;
    nan[12] = f32::NAN;
    let mut projective = SHIFT_X;
    projective[15] = 2.0;
    let mut singular = SHIFT_X;
    singular[0] = 0.0;
    for bad in [nan, projective, singular] {
        assert!(scene.place(StructureId(1), bad).is_err());
    }
    assert!(scene.place(StructureId(9), SHIFT_X).is_err());
    assert_eq!(scene.to_spec(), before);
}

const ASSEMBLY_CIF: &str = "data_demo
loop_
_atom_site.id
_atom_site.type_symbol
_atom_site.label_atom_id
_atom_site.label_comp_id
_atom_site.label_asym_id
_atom_site.label_seq_id
_atom_site.auth_seq_id
_atom_site.auth_asym_id
_atom_site.Cartn_x
_atom_site.Cartn_y
_atom_site.Cartn_z
1 C CA GLY A 1 1 A 1 0 0
2 C CA GLY B 1 1 B 0 2 0
loop_
_pdbx_struct_oper_list.id
_pdbx_struct_oper_list.matrix[1][1]
_pdbx_struct_oper_list.matrix[1][2]
_pdbx_struct_oper_list.matrix[1][3]
_pdbx_struct_oper_list.vector[1]
_pdbx_struct_oper_list.matrix[2][1]
_pdbx_struct_oper_list.matrix[2][2]
_pdbx_struct_oper_list.matrix[2][3]
_pdbx_struct_oper_list.vector[2]
_pdbx_struct_oper_list.matrix[3][1]
_pdbx_struct_oper_list.matrix[3][2]
_pdbx_struct_oper_list.matrix[3][3]
_pdbx_struct_oper_list.vector[3]
I 1 0 0 0 0 1 0 0 0 0 1 0
S 1 0 0 30 0 1 0 0 0 0 1 0
_pdbx_struct_assembly.id 1
loop_
_pdbx_struct_assembly_gen.assembly_id
_pdbx_struct_assembly_gen.oper_expression
_pdbx_struct_assembly_gen.asym_id_list
1 I A,B
1 S A
";

#[test]
fn an_assembly_becomes_one_placed_copy_per_transform_with_its_own_chains() {
    let structure = match molframe::read_bytes(
        ASSEMBLY_CIF.as_bytes().to_vec(),
        Some("assembly.cif"),
        &molframe::ReadOptions::new(),
    ) {
        Ok((structure, _)) => structure,
        Err(findings) => panic!("the fixture reads: {findings:?}"),
    };
    let mut scene = Scene::from_structure(&structure).unwrap_or_else(|error| panic!("{error}"));
    let copies = scene
        .add_assembly(StructureId(1), &structure, "1")
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(copies.len(), 2);
    let chains: Vec<Vec<&str>> = copies
        .iter()
        .map(|copy| copy.chains.iter().map(AsRef::as_ref).collect())
        .collect();
    assert_eq!(chains, vec![vec!["A", "B"], vec!["A"]]);
    assert_eq!(copies[1].selection.source(), "label_chain A");
    let shifted = scene.spec().structures[&copies[1].structure].placement;
    assert!(shifted.is_some_and(|matrix| (matrix[12] - 30.0).abs() < 1e-6));
    // Each copy draws only its own chains.
    for copy in &copies {
        scene
            .add(rep::spacefill(copy.selection.clone()).structure(copy.structure))
            .unwrap_or_else(|error| panic!("{error}"));
    }
    assert!(scene.add_assembly(StructureId(1), &structure, "9").is_err());
    assert!(scene.add_assembly(StructureId(7), &structure, "1").is_err());
}

fn shifted(x: f32) -> [f32; 16] {
    let mut matrix = SHIFT_X;
    matrix[12] = x;
    matrix
}

fn world_x(scene: &Scene) -> Vec<f32> {
    scene
        .resolved()
        .structures()
        .map(|(_, placed)| placed.model_to_world.w_axis.x)
        .collect()
}

#[test]
fn place_many_is_one_revision_and_matches_placing_one_by_one() {
    let matrices: Vec<[f32; 16]> = (0..6_u8)
        .map(|i| shifted(10.0 * f32::from(i) + 1.0))
        .collect();
    let mut one_by_one = scene();
    one_by_one
        .add(rep::spacefill(sel::all()))
        .unwrap_or_else(|error| panic!("{error}"));
    let mut batched = scene();
    batched
        .add(rep::spacefill(sel::all()))
        .unwrap_or_else(|error| panic!("{error}"));
    let revision = batched.revision();
    let singles: Vec<_> = matrices
        .iter()
        .map(|matrix| {
            one_by_one
                .place(StructureId(1), *matrix)
                .unwrap_or_else(|error| panic!("{error}"))
        })
        .collect();
    let many = batched
        .place_many(StructureId(1), &matrices)
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(many, singles);
    assert_eq!(batched.revision(), revision + 1);
    assert_eq!(one_by_one.revision(), revision + 6);
    assert_eq!(batched.spec().structures, one_by_one.spec().structures);
    assert_eq!(world_x(&batched), world_x(&one_by_one));
    // The copies stay usable: represent one and read its selection.
    batched
        .add(rep::spacefill(sel::all()).structure(many[5]))
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(batched.resolved().structures().count(), 7);
}

#[test]
fn a_failed_batch_changes_nothing() {
    let mut scene = scene();
    let before = scene.to_spec();
    let mut bad = SHIFT_X;
    bad[0] = 0.0;
    assert!(scene.place_many(StructureId(1), &[SHIFT_X, bad]).is_err());
    assert_eq!(scene.to_spec(), before);
    assert_eq!(scene.resolved().structures().count(), 1);
    let id = scene
        .place(StructureId(1), SHIFT_X)
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(id, StructureId(2));
}

#[test]
fn copies_added_after_a_focus_still_resolve_the_whole_scene() {
    let mut scene = scene();
    scene
        .add(rep::spacefill(sel::all()))
        .unwrap_or_else(|error| panic!("{error}"));
    let _ = scene
        .place(StructureId(1), SHIFT_X)
        .unwrap_or_else(|error| panic!("{error}"));
    scene
        .focus(sel::all())
        .unwrap_or_else(|error| panic!("{error}"));
    let slow = scene
        .place_many(StructureId(1), &[shifted(40.0), shifted(60.0)])
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(slow, vec![StructureId(3), StructureId(4)]);
    assert_eq!(world_x(&scene), vec![0.0, 25.0, 40.0, 60.0]);
}

#[test]
fn the_content_hash_is_computed_once_per_source() {
    let mut scene = scene();
    let _ = scene
        .place_many(StructureId(1), &[SHIFT_X, shifted(3.0)])
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(scene.content_hashes.len(), 1);
    let hash = scene.spec().structures[&StructureId(1)]
        .content_hash
        .clone();
    assert_eq!(scene.spec().structures[&StructureId(3)].content_hash, hash);
}

#[test]
fn removing_a_structure_unbinds_it_and_keeps_the_others_drawable() {
    let mut scene = scene();
    let original = StructureId(1);
    let copy = scene
        .place(original, SHIFT_X)
        .unwrap_or_else(|error| panic!("{error}"));
    scene
        .add(rep::spacefill(sel::all()).structure(copy))
        .unwrap_or_else(|error| panic!("{error}"));
    let before = scene.revision();
    scene
        .remove_structure(original)
        .unwrap_or_else(|error| panic!("{error}"));
    assert!(scene.revision() > before);
    assert!(!scene.spec().structures.contains_key(&original));
    assert!(scene.spec().structures.contains_key(&copy));
    assert_eq!(scene.resolved().structures().count(), 1);
    assert!(scene.remove_structure(original).is_err(), "already gone");
}

#[test]
fn a_structure_still_drawn_cannot_be_removed_and_the_scene_is_unchanged() {
    let mut scene = scene();
    let original = StructureId(1);
    scene
        .add(rep::spacefill(sel::all()).structure(original))
        .unwrap_or_else(|error| panic!("{error}"));
    let before = scene.revision();
    assert!(scene.remove_structure(original).is_err());
    assert_eq!(scene.revision(), before);
    assert!(scene.spec().structures.contains_key(&original));
    assert_eq!(scene.resolved().structures().count(), 1);
}
