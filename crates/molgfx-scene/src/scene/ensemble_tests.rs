use super::*;
use std::fmt::Write as _;

fn model(number: u32, atoms: u32, shift: f32) -> String {
    let mut text = format!("MODEL     {number:>4}\n");
    for index in 0..atoms {
        let x = f32::from(u16::try_from(index).unwrap_or(0)) * 1.45 + shift;
        let name = if index == 0 { " N  " } else { " CA " };
        let element = if index == 0 { "N" } else { "C" };
        let _ = writeln!(
            text,
            "ATOM  {:>5} {name} GLY A   1    {x:>8.3}   0.000   0.000  1.00 10.00          {element:>2}",
            index + 1
        );
    }
    text.push_str("TER\nENDMDL\n");
    text
}

fn read(text: String) -> molframe::Structure {
    let Ok((structure, _)) = molframe::read_bytes(
        text.into_bytes(),
        Some("m.pdb"),
        &molframe::ReadOptions::new(),
    ) else {
        panic!("fixture parses")
    };
    structure
}

fn atoms(scene: &Scene) -> usize {
    scene
        .sources()
        .values()
        .map(|source| source.coordinates().len())
        .sum()
}

#[test]
fn a_ragged_ensemble_draws_its_first_model_instead_of_an_empty_shell() {
    let structure = read(format!("{}{}END\n", model(1, 2, 0.0), model(2, 3, 0.0)));
    assert!(structure.is_ragged_ensemble());
    let scene = Scene::from_structure(&structure).unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(atoms(&scene), 2);
}

#[test]
fn a_ragged_ensemble_model_can_be_selected_by_index() {
    let structure = read(format!("{}{}END\n", model(1, 2, 0.0), model(2, 3, 0.0)));
    let scene = Scene::from_structure_model(&structure, 1).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(atoms(&scene), 3);
}

#[test]
fn a_dense_ensemble_model_uses_that_models_coordinates() {
    let structure = read(format!("{}{}END\n", model(1, 2, 0.0), model(2, 2, 10.0)));
    assert!(!structure.is_ragged_ensemble());
    assert_eq!(structure.model_count(), 2);
    let scene = Scene::from_structure_model(&structure, 1).unwrap_or_else(|e| panic!("{e}"));
    let first = scene.sources().values().next().map(|s| s.coordinates()[0]);
    assert_eq!(first, Some([10.0, 0.0, 0.0]));
}

#[test]
fn an_out_of_range_model_is_an_error_naming_the_count() {
    let structure = read(format!("{}{}END\n", model(1, 2, 0.0), model(2, 3, 0.0)));
    let Err(error) = Scene::from_structure_model(&structure, 5) else {
        panic!("model 5 does not exist")
    };
    assert!(error.to_string().contains("2 models"), "{error}");
}

#[test]
fn a_ragged_shell_cannot_be_bound_through_a_spec_silently() {
    let structure = read(format!("{}{}END\n", model(1, 2, 0.0), model(2, 3, 0.0)));
    let scene = Scene::from_structure(&structure).unwrap_or_else(|e| panic!("{e}"));
    let spec = scene.to_spec();
    let id = scene
        .primary_structure()
        .unwrap_or_else(|| panic!("has one"));
    let bound = std::collections::BTreeMap::from([(id, structure)]);
    assert!(Scene::from_spec(spec, bound).is_err());
}

#[test]
fn structure_accessors_name_the_structure_a_scene_was_built_with() {
    let structure = read(model(1, 2, 0.0));
    let (scene, id) = Scene::from_structure_with_id(&structure).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(scene.primary_structure(), Some(id));
    assert_eq!(scene.structure_ids().collect::<Vec<_>>(), vec![id]);
}

#[test]
fn an_empty_structure_is_refused_with_guidance() {
    // The reader itself refuses an atom-less file; when it does accept one,
    // the scene must still refuse to build a blank image.
    let Ok((structure, _)) = molframe::read_bytes(
        b"END\n".to_vec(),
        Some("e.pdb"),
        &molframe::ReadOptions::new(),
    ) else {
        return;
    };
    let Err(error) = Scene::from_structure(&structure) else {
        panic!("an empty scene must not build")
    };
    assert!(error.to_string().contains("no atoms"), "{error}");
}
