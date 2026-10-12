use crate::{ColorSpec, Scene, rep};

fn ion() -> molframe::Structure {
    let pdb =
        b"HETATM    1 MG    MG A   1       0.000   0.000   0.000  1.00 20.00          MG\nEND\n";
    molframe::read_bytes(pdb.to_vec(), Some("ion.pdb"), &molframe::ReadOptions::new())
        .unwrap()
        .0
}

#[test]
fn unsupported_forms_report_the_missing_target_geometry() {
    let scene = Scene::from_structure(&ion()).unwrap();
    let forms: [crate::RepresentationSpec; 3] = [
        rep::cartoon("all").into(),
        rep::trace("all").into(),
        rep::bonds("all").into(),
    ];
    for form in forms {
        let applicability = scene.representation_applicability(form).unwrap();
        assert_eq!(applicability.selected_atoms, 1);
        assert!(!applicability.is_applicable());
    }
    assert!(
        scene
            .representation_applicability(rep::lines("all"))
            .unwrap()
            .is_applicable()
    );
    assert!(
        scene
            .representation_applicability(rep::spacefill("all"))
            .unwrap()
            .is_applicable()
    );
    assert!(
        scene
            .representation_applicability(rep::licorice("all"))
            .unwrap()
            .is_applicable()
    );
}

#[test]
fn default_polyhedra_draws_a_selected_metal_center_with_unselected_coordination_neighbors() {
    let pdb = b"HETATM    1 MG    MG A   1       0.000   0.000   0.000  1.00 20.00          MG\nHETATM    2  O   HOH A   2       2.100   0.000   0.000  1.00 20.00           O\nHETATM    3  O   HOH A   3       0.000   2.100   0.000  1.00 20.00           O\nHETATM    4  O   HOH A   4       0.000   0.000   2.100  1.00 20.00           O\nEND\n";
    let (source, _) = molframe::read_bytes(
        pdb.to_vec(),
        Some("coordination.pdb"),
        &molframe::ReadOptions::new(),
    )
    .unwrap();
    let scene = Scene::from_structure(&source).unwrap();
    assert!(
        scene
            .representation_applicability(rep::polyhedra("element Mg"))
            .unwrap()
            .is_applicable()
    );
    assert!(
        !scene
            .representation_applicability(rep::polyhedra("element Mg").cutoff(1.6))
            .unwrap()
            .is_applicable()
    );
}

#[test]
fn a_failed_form_change_preserves_the_entire_layer_and_scene_revision() {
    let mut scene = Scene::from_structure(&ion()).unwrap();
    let id = scene
        .add(rep::spacefill("all").color(ColorSpec::Element).opacity(0.4))
        .unwrap();
    scene.set_visible(id, false).unwrap();
    let before = scene.to_spec();
    assert!(
        scene
            .set_representation_form(id, rep::cartoon("all"))
            .is_err()
    );
    assert_eq!(scene.spec(), &before);
}

#[test]
fn a_successful_form_change_keeps_identity_target_and_authored_appearance() {
    let mut scene = Scene::from_structure(&ion()).unwrap();
    let id = scene
        .add(rep::spacefill("all").color(ColorSpec::Element).opacity(0.4))
        .unwrap();
    scene.set_visible(id, false).unwrap();
    let before = scene.spec().representations[&id].clone();
    scene
        .set_representation_form(id, rep::points("none").size(2.0))
        .unwrap();
    let after = &scene.spec().representations[&id];
    assert_eq!(after.form_name(), "points");
    assert_eq!(after.target(), before.target());
    assert_eq!(after.color(), before.color());
    assert_eq!(after.opacity().to_bits(), before.opacity().to_bits());
    assert_eq!(after.is_visible(), before.is_visible());
    assert_eq!(scene.spec().representations.len(), 1);
}

#[test]
fn phosphate_only_nucleic_targets_are_available_to_trace_and_tube() {
    let pdb = b"ATOM      1  P    DA A   1       0.000   0.000   0.000  1.00 10.00           P\nATOM      2  C4'  DA A   1       1.000   0.000   0.000  1.00 10.00           C\nATOM      3  P    DT A   2       4.000   0.000   0.000  1.00 10.00           P\nATOM      4  C4'  DT A   2       5.000   0.000   0.000  1.00 10.00           C\nEND\n";
    let (structure, _) = molframe::read_bytes(
        pdb.to_vec(),
        Some("backbone.pdb"),
        &molframe::ReadOptions::new(),
    )
    .expect("nucleic source");
    let scene = Scene::from_structure(&structure).expect("scene");
    let forms: [crate::RepresentationSpec; 3] = [
        rep::trace("name P").into(),
        rep::tube("name P").into(),
        rep::backbone("name P").into(),
    ];
    for form in forms {
        assert!(
            scene
                .representation_applicability(form)
                .expect("target review")
                .is_applicable()
        );
    }
    assert!(
        !scene
            .representation_applicability(rep::cartoon("name P"))
            .expect("sugar trace review")
            .is_applicable()
    );
}
