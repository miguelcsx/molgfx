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

#[test]
fn a_framed_atom_is_not_cropped() {
    let scene = two_chains();
    let camera = scene
        .frame("name N and chain A", 1.0)
        .unwrap_or_else(|error| panic!("{error}"));
    // The nitrogen at the origin has a 1.55 A radius: the points a radius
    // above and below its centre, across the screen, must both be on screen.
    let forward = (camera.target - camera.eye).normalize();
    let up = forward.cross(camera.up).cross(forward).normalize();
    let size = (400, 400);
    for sign in [-1.0f32, 1.0] {
        let edge = Vec3::ZERO + up * (1.55 * sign);
        let Some(point) = camera.project(edge, size) else {
            panic!("the edge is in front of the camera")
        };
        assert!(
            (0.0..400.0).contains(&point.y),
            "the atom's edge lands at y = {}",
            point.y
        );
    }
}
