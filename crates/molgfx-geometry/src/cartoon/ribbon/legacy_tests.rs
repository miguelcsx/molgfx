use super::*;
use molgfx_core::AtomSelection;

fn source() -> molframe::Structure {
    let pdb = b"ATOM      1  CA  ALA A   1      -4.000   0.000   0.000  1.00 10.00           C\nATOM      2  O   ALA A   1      -4.000   1.500   0.000  1.00 10.00           O\nATOM      3  CA  ALA A   2       0.000   0.000   0.000  1.00 10.00           C\nATOM      4  O   ALA A   2       0.000   1.500   0.000  1.00 10.00           O\nATOM      5  CA  ALA A   3       4.000   0.000   0.000  1.00 10.00           C\nATOM      6  O   ALA A   3       4.000   1.500   0.000  1.00 10.00           O\nEND\n";
    molframe::read_bytes(
        pdb.to_vec(),
        Some("ribbons.pdb"),
        &molframe::ReadOptions::new(),
    )
    .unwrap()
    .0
}

#[test]
fn independently_splined_borders_bound_a_sheet_and_circular_tubes() {
    let mut mesh = RibbonMesh::default();
    let params = RibbonParams {
        width: 2.0,
        max_steps: 4,
        ..RibbonParams::default()
    };
    mesh.generate_legacy_ribbons(&source(), &AtomSelection::All, 8.0, params, 0.2)
        .unwrap();
    let left = Vec3::from(mesh.vertices[0].position);
    let right = Vec3::from(mesh.vertices[1].position);
    assert!((left.distance(right) - 2.0).abs() < 1e-6);
    for vertex in &mesh.vertices[2..18] {
        assert!((Vec3::from(vertex.position).distance(left) - 0.2).abs() < 1e-6);
        assert!(Vec3::from(vertex.normal).is_normalized());
    }
    assert!(
        mesh.indices
            .iter()
            .all(|index| (*index as usize) < mesh.vertices.len())
    );
    assert_eq!(mesh.vertices.len(), mesh.deformations.len());
    assert!(
        mesh.deformations
            .iter()
            .all(|recipe| recipe.parameter[3].to_bits() == 9)
    );
    assert!(
        mesh.vertices
            .iter()
            .all(|vertex| [0, 2, 4].contains(&vertex.entity_id))
    );
}

#[test]
fn a_zero_border_radius_keeps_only_the_two_sided_sheet() {
    let mut mesh = RibbonMesh::default();
    let params = RibbonParams {
        width: 2.0,
        max_steps: 4,
        ..RibbonParams::default()
    };
    mesh.generate_legacy_ribbons(&source(), &AtomSelection::All, 8.0, params, 0.0)
        .unwrap();
    assert_eq!(mesh.vertices.len(), 20);
    assert_eq!(mesh.indices.len(), 48);
    assert!(
        mesh.vertices
            .iter()
            .all(|vertex| vertex.position[2].abs() < 1e-6)
    );
}

#[test]
fn a_selected_residue_keeps_both_border_spline_halves_and_owns_every_cap() {
    for radius in [0.0, 0.2] {
        let mut mesh = RibbonMesh::default();
        mesh.generate_legacy_ribbons(
            &source(),
            &AtomSelection::Range(2..3),
            8.0,
            RibbonParams {
                width: 2.0,
                max_steps: 4,
                ..RibbonParams::default()
            },
            radius,
        )
        .expect("selected ribbon");
        assert!(!mesh.indices.is_empty());
        assert!(mesh.vertices.iter().all(|vertex| vertex.entity_id == 2));
        assert!((mesh.vertices[0].position[0] + 2.25).abs() < 1e-6);
        assert!(
            mesh.vertices
                .iter()
                .all(|vertex| vertex.position[0].abs() <= 2.25001)
        );
        assert_eq!(mesh.deformations[0].controls, [0, 0, 2, 4]);
        assert_eq!(
            mesh.deformations.last().expect("last recipe").controls,
            [0, 2, 4, 4]
        );
        assert_eq!(
            mesh.deformations[0].parameter[0].to_bits(),
            0.5_f32.to_bits()
        );
        assert_eq!(
            mesh.deformations.last().expect("last recipe").parameter[0].to_bits(),
            0.5_f32.to_bits()
        );
    }
}

#[test]
fn a_ribbon_control_has_identical_width_cues_in_adjacent_intervals() {
    let mut mesh = RibbonMesh::default();
    mesh.generate_legacy_ribbons(
        &source(),
        &AtomSelection::All,
        8.0,
        RibbonParams::default(),
        0.0,
    )
    .unwrap();
    let first_offset = mesh.deformations[0].parameter[1].to_bits() as usize;
    let second_offset = mesh.deformations[18].parameter[1].to_bits() as usize;
    let stream = mesh.radius_source_values();
    // The middle source guide is lane two in interval zero and lane one in
    // interval one. Its neighbors, oxygen and sign must match exactly.
    assert_eq!(
        &stream[first_offset + 4..first_offset + 6],
        &stream[second_offset + 2..second_offset + 4]
    );
    assert_eq!(
        stream[first_offset + 10].to_bits(),
        stream[second_offset + 9].to_bits()
    );
    assert_eq!(
        stream[first_offset + 14].to_bits(),
        stream[second_offset + 13].to_bits()
    );
}
