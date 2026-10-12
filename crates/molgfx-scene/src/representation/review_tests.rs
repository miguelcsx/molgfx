use crate::{Scene, StructureId, TrajectoryFrame, rep};
use std::sync::Arc;

#[test]
fn an_isolated_polymer_selection_is_applicable_when_its_parent_supplies_spline_controls() {
    let source = molframe::read_bytes(
        b"ATOM      1  CA  ALA A   1      -3.000   0.000   0.000  1.00 10.00           C\nATOM      2  CA  ALA A   2       0.000   2.000   0.000  1.00 10.00           C\nATOM      3  CA  ALA A   3       3.000   0.000   0.000  1.00 10.00           C\nEND\n".to_vec(),
        Some("selected-polymer.pdb"), &molframe::ReadOptions::new(),
    ).expect("source").0;
    let mut scene = Scene::from_structure(&source).expect("scene");
    let forms: [crate::RepresentationSpec; 3] = [
        rep::new_cartoon("resid 2").into(),
        rep::new_ribbons("resid 2").into(),
        rep::ribbons("resid 2").into(),
    ];
    for form in forms {
        assert!(
            scene
                .representation_applicability(form.clone())
                .expect("review")
                .is_applicable()
        );
        scene.add(form).expect("supported selected curve");
    }
}

fn scene() -> Scene {
    let pdb = b"HETATM    1 MG    MG A   1       0.000   0.000   0.000  1.00 20.00          MG\nHETATM    2  O   HOH A   2       4.000   0.000   0.000  1.00 20.00           O\nHETATM    3  O   HOH A   3       0.000   4.000   0.000  1.00 20.00           O\nHETATM    4  O   HOH A   4       0.000   0.000   4.000  1.00 20.00           O\nEND\n";
    let structure = molframe::read_bytes(
        pdb.to_vec(),
        Some("coordination.pdb"),
        &molframe::ReadOptions::new(),
    )
    .expect("source")
    .0;
    Scene::from_structure(&structure).expect("scene")
}

#[test]
fn protein_cartoon_requires_alpha_carbons_while_new_cartoon_accepts_nucleic_guides() {
    let source = molframe::read_bytes(
        b"ATOM      1  C4'  DA A   1      -3.000   0.000   0.000  1.00 10.00           C\nATOM      2  C4'  DA A   2       0.000   2.000   0.000  1.00 10.00           C\nATOM      3  C4'  DA A   3       3.000   0.000   0.000  1.00 10.00           C\nEND\n".to_vec(),
        Some("nucleic.pdb"), &molframe::ReadOptions::new(),
    ).expect("source").0;
    let scene = Scene::from_structure(&source).expect("scene");
    assert!(
        !scene
            .representation_applicability(rep::cartoon("all"))
            .expect("review")
            .is_applicable()
    );
    assert!(
        scene
            .representation_applicability(rep::new_cartoon("all"))
            .expect("review")
            .is_applicable()
    );
}

#[test]
fn distance_reviews_use_the_current_pose_and_keep_it_after_later_playback() {
    let mut scene = scene();
    let id = StructureId::new(1);
    let far = Arc::from([[0.0; 3], [4.0, 0.0, 0.0], [0.0, 4.0, 0.0], [0.0, 0.0, 4.0]]);
    let near = Arc::from([[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);
    scene
        .set_trajectory_pair(
            id,
            TrajectoryFrame::new(0, 0.0, far),
            TrajectoryFrame::new(1, 1.0, near),
            0.0,
        )
        .expect("interval");
    let forms: [crate::RepresentationSpec; 2] = [
        rep::dynamic_bonds("all").cutoff(3.0).into(),
        rep::polyhedra("element Mg").into(),
    ];
    for form in &forms {
        assert!(
            !scene
                .representation_applicability(form.clone())
                .expect("far pose")
                .is_applicable()
        );
    }
    scene.set_trajectory_time(id, 0.5).expect("advance");
    let key = scene.trajectory_pose_key(id).expect("pose identity");
    let captured = scene.representation_review(id).expect("capture");
    for form in &forms {
        assert!(
            scene
                .representation_applicability(form.clone())
                .expect("near pose")
                .is_applicable()
        );
    }
    scene
        .add(rep::spacefill("all"))
        .expect("edit without changing pose");
    assert_eq!(scene.trajectory_pose_key(id), Some(key));
    scene.clear_trajectory_pair(id);
    let reviewed = std::thread::spawn(move || {
        for form in forms {
            assert!(
                captured
                    .applicability(&form)
                    .expect("retained pose")
                    .is_applicable()
            );
        }
    });
    reviewed.join().expect("worker");
    assert!(
        !scene
            .representation_applicability(rep::polyhedra("element Mg"))
            .expect("restored source")
            .is_applicable()
    );
}

#[test]
fn polyhedra_reviews_reject_a_triangle_that_collapses_at_the_sampled_pose() {
    let mut scene = scene();
    let id = StructureId::new(1);
    let start = Arc::from([[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);
    let end = Arc::from([[0.0; 3], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0], [3.0, 0.0, 0.0]]);
    scene
        .set_trajectory_pair(
            id,
            TrajectoryFrame::new(0, 0.0, start),
            TrajectoryFrame::new(1, 1.0, end),
            1.0,
        )
        .expect("interval");
    assert!(
        !scene
            .representation_applicability(rep::polyhedra("element Mg"))
            .expect("collapsed face")
            .is_applicable()
    );
    scene.set_trajectory_time(id, 0.0).expect("reverse");
    assert!(
        scene
            .representation_applicability(rep::polyhedra("element Mg"))
            .expect("nondegenerate face")
            .is_applicable()
    );
}

#[test]
fn surface_opacity_reaches_the_resolved_representation_and_routes_it_translucent() {
    let source = molframe::read_bytes(
        b"HETATM    1  N   LIG A   1       0.000   0.000  -4.000  1.00 10.00           N\nHETATM    2  O   HOH A   2       0.000   0.000   0.000  1.00 10.00           O\nEND\n".to_vec(),
        Some("surface.pdb"), &molframe::ReadOptions::new(),
    ).expect("source").0;
    let mut scene = Scene::from_structure(&source).expect("scene");
    scene
        .add(rep::surface("all").opacity(0.3))
        .expect("surface");
    let translucent: Vec<_> = scene
        .resolved()
        .representations()
        .map(|(_, representation)| {
            (
                representation.material.opacity,
                representation.is_translucent(),
            )
        })
        .collect();
    assert_eq!(translucent, vec![(0.3, true)]);
}
