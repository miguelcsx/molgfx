use super::*;

fn scene() -> Scene {
    let source = "\
data_query
loop_
_entity.id
_entity.type
1 polymer
2 non-polymer
3 water
loop_
_entity_poly.entity_id
_entity_poly.type
1 polypeptide(L)
loop_
_struct_asym.id
_struct_asym.entity_id
A 1
L 2
W 3
loop_
_atom_site.group_PDB
_atom_site.id
_atom_site.type_symbol
_atom_site.label_atom_id
_atom_site.label_comp_id
_atom_site.label_asym_id
_atom_site.label_entity_id
_atom_site.label_seq_id
_atom_site.Cartn_x
_atom_site.Cartn_y
_atom_site.Cartn_z
ATOM 1 C CA GLY A 1 1 0 0 0
HETATM 2 C C1 LIG L 2 . 1 0 0
HETATM 3 O O HOH W 3 . 5 0 0
";
    let structure = match molframe::read_bytes(
        source.as_bytes().to_vec(),
        Some("query.cif"),
        &molframe::ReadOptions::default(),
    ) {
        Ok((structure, _)) => structure,
        Err(diagnostics) => panic!("query fixture parses: {diagnostics:?}"),
    };
    match Scene::from_structure(&structure) {
        Ok(scene) => scene,
        Err(error) => panic!("query scene builds: {error}"),
    }
}

fn rows(scene: &Scene, selection: SelectionHandle) -> Vec<u32> {
    let Some(selection) = scene.selection(selection) else {
        panic!("selection resolves")
    };
    selection.to_bitmap(3).iter().collect()
}

#[test]
fn entity_queries_follow_declared_pdbx_semantics() {
    let mut scene = scene();
    let polymer = match scene.select(Select::polymer()) {
        Ok(selection) => selection,
        Err(error) => panic!("polymer query executes: {error}"),
    };
    let ligand = match scene.select(Select::ligands()) {
        Ok(selection) => selection,
        Err(error) => panic!("ligand query executes: {error}"),
    };
    let water = match scene.select(Select::water()) {
        Ok(selection) => selection,
        Err(error) => panic!("water query executes: {error}"),
    };
    assert_eq!(rows(&scene, polymer), vec![0]);
    assert_eq!(rows(&scene, ligand), vec![1]);
    assert_eq!(rows(&scene, water), vec![2]);
}

#[test]
fn absent_polymer_classification_is_never_guessed_from_component_names() {
    let mut scene = scene();
    let protein = match scene.select(Select::protein()) {
        Ok(selection) => selection,
        Err(error) => panic!("protein query executes: {error}"),
    };
    assert!(rows(&scene, protein).is_empty());
}

#[test]
fn string_and_builder_queries_execute_through_one_ir() {
    let mut scene = scene();
    let string = match scene.select_str("protein or ligand") {
        Ok(selection) => selection,
        Err(error) => panic!("string query executes: {error}"),
    };
    let builder = match scene.select(Select::protein().or(Select::ligands())) {
        Ok(selection) => selection,
        Err(error) => panic!("builder query executes: {error}"),
    };
    assert_eq!(rows(&scene, string), rows(&scene, builder));
}

#[test]
fn spatial_ir_uses_bvh_candidates_then_exact_world_distance() {
    let mut scene = scene();
    let selection = match scene.select_str("within 1.1 of ligand") {
        Ok(selection) => selection,
        Err(error) => panic!("spatial query executes: {error}"),
    };
    assert_eq!(rows(&scene, selection), vec![0, 1]);
}

#[test]
fn hierarchy_property_and_geometric_predicates_execute_against_source_columns() {
    let source = crate::fixture::structure();
    let mut scene = match Scene::from_structure(&source) {
        Ok(scene) => scene,
        Err(error) => panic!("fixture scene: {error}"),
    };
    let chain = scene
        .select_str("chain A and resname GLY")
        .expect("chain query");
    assert_eq!(rows(&scene, chain), vec![0, 1, 2, 3, 4, 5]);
    let carbon = scene.select_str("element C").expect("element query");
    assert_eq!(rows(&scene, carbon), vec![1, 2]);
    let high_b = scene.select_str("b_factor >= 20").expect("B-factor query");
    assert_eq!(rows(&scene, high_b), vec![6, 7]);
    let hydrogen = scene.select(Select::hydrogen()).expect("hydrogen query");
    assert_eq!(rows(&scene, hydrogen), vec![4]);
    let sphere = scene
        .select_str("in_sphere 0 0 0 0.1")
        .expect("sphere query");
    assert_eq!(rows(&scene, sphere), vec![1]);
    let box_selection = scene
        .select_str("in_box -1 -1 -1 2 2 1")
        .expect("box query");
    assert_eq!(rows(&scene, box_selection), vec![0, 1, 2]);
}
