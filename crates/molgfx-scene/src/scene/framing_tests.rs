use super::*;
use molgfx_math::Vec3;

fn two_chains() -> Scene {
    const PDB: &str = "\
ATOM      1  N   ALA A   1       0.000   0.000   0.000  1.00  0.00           N
ATOM      2  CA  ALA A   1       2.000   0.000   0.000  1.00  0.00           C
ATOM      3  N   GLY B   1      30.000  10.000   0.000  1.00  0.00           N
ATOM      4  CA  GLY B   1      34.000  10.000   0.000  1.00  0.00           C
END
";
    let Ok((structure, _)) = molframe::read_bytes(
        PDB.as_bytes().to_vec(),
        Some("two.pdb"),
        &molframe::ReadOptions::new(),
    ) else {
        panic!("fixture parses")
    };
    Scene::from_structure(&structure).unwrap_or_else(|error| panic!("{error}"))
}

#[test]
fn bounds_cover_exactly_the_atoms_a_selection_picks() {
    let scene = two_chains();
    let bounds = scene
        .selection_bounds("chain B")
        .unwrap_or_else(|error| panic!("{error}"))
        .unwrap_or_else(|| panic!("chain B has atoms"));
    assert_eq!(bounds.min, Vec3::new(30.0, 10.0, 0.0));
    assert_eq!(bounds.max, Vec3::new(34.0, 10.0, 0.0));
}

#[test]
fn a_selection_that_picks_nothing_has_no_bounds_and_cannot_be_framed() {
    let scene = two_chains();
    assert!(
        scene
            .selection_bounds("chain Z")
            .unwrap_or_else(|error| panic!("{error}"))
            .is_none()
    );
    assert!(scene.frame("chain Z", 1.0).is_err());
}

#[test]
fn the_frame_looks_at_the_selection_and_leaves_the_scene_alone() {
    let scene = two_chains();
    let revision = scene.revision();
    let camera = scene
        .frame("chain B", 1.5)
        .unwrap_or_else(|error| panic!("{error}"));
    assert!((camera.target - Vec3::new(32.0, 10.0, 0.0)).length() < 1e-4);
    assert_eq!(scene.revision(), revision);
    let whole = scene.framing_camera(1.5);
    assert!(
        camera.eye.distance(camera.target) < whole.eye.distance(whole.target),
        "framing a part comes closer than framing the whole"
    );
}

#[test]
fn an_invalid_query_is_an_error_not_an_empty_frame() {
    assert!(two_chains().frame("chain (", 1.0).is_err());
}
