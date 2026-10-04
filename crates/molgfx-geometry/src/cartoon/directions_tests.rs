use crate::{CartoonError, RibbonMesh, RibbonParams};
use molgfx_core::{AtomSelection, EntityId, EntityKind};
use molgfx_math::Vec3;

fn structure(singleton: bool, backbone: bool) -> molframe::Structure {
    let data = if singleton && backbone {
        "ATOM      1  N   GLY A   1      -1.000   0.000   0.000  1.00 10.00           N\nATOM      2  CA  GLY A   1       0.000   0.000   0.000  1.00 10.00           C\nATOM      3  C   GLY A   1       1.000   0.000   0.000  1.00 10.00           C\nEND\n"
    } else if singleton {
        "ATOM      1  CA  GLY A   1       0.000   0.000   0.000  1.00 10.00           C\nEND\n"
    } else {
        "ATOM      1  CA  GLY A   1       0.000   0.000   0.000  1.00 10.00           C\nATOM      2  CA  GLY A   2       2.000   0.000   0.000  1.00 10.00           C\nATOM      3  CA  GLY A   3       4.000   0.000   0.000  1.00 10.00           C\nEND\n"
    };
    molframe::read_bytes(
        data.as_bytes().to_vec(),
        Some("guides.pdb"),
        &molframe::ReadOptions::new(),
    )
    .expect("source guides parse")
    .0
}

fn params() -> RibbonParams {
    RibbonParams {
        direction_wedges: true,
        ..RibbonParams::default()
    }
}

#[test]
fn every_selected_guide_including_the_terminal_one_gets_one_forward_wedge() {
    let source = structure(false, false);
    let mut plain = RibbonMesh::default();
    plain
        .generate_structure(
            &source,
            &AtomSelection::All,
            &[],
            8.0,
            RibbonParams::default(),
        )
        .expect("ribbon");
    let mut marked = RibbonMesh::default();
    marked
        .generate_structure(&source, &AtomSelection::All, &[], 8.0, params())
        .expect("wedges");
    assert_eq!(marked.vertices.len(), plain.vertices.len() + 9);
    assert_eq!(marked.indices.len(), plain.indices.len() + 9);
    assert_eq!(&marked.vertices[..plain.vertices.len()], &plain.vertices);
    for (guide, triangle) in marked.vertices[plain.vertices.len()..]
        .as_chunks::<3>()
        .0
        .iter()
        .enumerate()
    {
        let source_x = f32::from(u8::try_from(guide).expect("three guides")) * 2.0;
        let positions = triangle.map(|vertex| Vec3::from(vertex.position));
        assert!((positions[0].x - source_x - 0.4).abs() < 1.0e-6);
        assert!((positions[1].x - source_x + 0.2).abs() < 1.0e-6);
        assert!((positions[1].distance(positions[2]) - 0.6).abs() < 1.0e-6);
        let normal = (positions[1] - positions[0]).cross(positions[2] - positions[0]);
        assert!(normal.dot(Vec3::from(triangle[0].normal)) > 0.0);
        let expected = EntityId::pack(EntityKind::Atom, guide as u64)
            .expect("guide identity")
            .0;
        assert!(triangle.iter().all(|vertex| vertex.entity_id == expected));
    }
}

#[test]
fn an_isolated_selection_uses_its_parent_polymer_direction_without_drawing_neighbors() {
    let mut mesh = RibbonMesh::default();
    mesh.generate_structure(
        &structure(false, false),
        &AtomSelection::Sparse(vec![1]),
        &[],
        8.0,
        params(),
    )
    .expect("parent direction");
    assert_eq!(mesh.vertices.len(), 3);
    assert_eq!(mesh.indices, [0, 1, 2]);
    assert!((mesh.vertices[0].position[0] - 2.4).abs() < 1.0e-6);
    assert!(
        mesh.vertices
            .iter()
            .all(|vertex| EntityId(vertex.entity_id).unpack() == Some((EntityKind::Atom, 1)))
    );
    let recipe: &[u32] = bytemuck::cast_slice(mesh.deformation_bytes());
    assert_eq!(&recipe[..4], &[0, 0, 1, 2]);
    assert_eq!(recipe[4], 1.0_f32.to_bits());
}

#[test]
fn a_single_residue_uses_its_backbone_atoms_and_keeps_a_live_guide_anchor() {
    let mut mesh = RibbonMesh::default();
    mesh.generate_structure(
        &structure(true, true),
        &AtomSelection::All,
        &[],
        8.0,
        params(),
    )
    .expect("backbone direction");
    assert_eq!(mesh.vertices.len(), 3);
    assert!((mesh.vertices[0].position[0] - 0.4).abs() < 1.0e-6);
    let recipe: &[u32] = bytemuck::cast_slice(mesh.deformation_bytes());
    assert_eq!(&recipe[..4], &[0, 1, 2, 1]);
    assert_eq!(recipe[7], 1);
}

#[test]
fn a_guide_without_a_source_direction_returns_a_typed_error_and_no_partial_mesh() {
    let mut mesh = RibbonMesh::default();
    mesh.generate_structure(
        &structure(false, false),
        &AtomSelection::All,
        &[],
        8.0,
        params(),
    )
    .expect("prior geometry");
    assert!(!mesh.vertices.is_empty());
    assert!(matches!(
        mesh.generate_structure(
            &structure(true, false),
            &AtomSelection::All,
            &[],
            8.0,
            params()
        ),
        Err(CartoonError::GuideDirection { .. })
    ));
    assert!(
        mesh.vertices.is_empty() && mesh.indices.is_empty() && mesh.deformation_bytes().is_empty()
    );
    mesh.generate_structure(
        &structure(true, false),
        &AtomSelection::Empty,
        &[],
        8.0,
        params(),
    )
    .expect("an unselected guide needs no direction");
    assert!(mesh.vertices.is_empty());
}

#[test]
fn coincident_parent_guides_do_not_invent_a_polymer_direction() {
    let mut mesh = RibbonMesh::default();
    assert!(matches!(
        mesh.generate(&[Vec3::ZERO, Vec3::ZERO], &[0, 1], params()),
        Err(CartoonError::GuideDirection { .. })
    ));
    assert!(mesh.vertices.is_empty() && mesh.indices.is_empty());
}

#[test]
fn an_isolated_nucleotide_uses_its_sugar_backbone_atoms() {
    let (source, _) = molframe::read_bytes(
        b"ATOM      1  C5'  DA A   1      -1.000   0.000   0.000  1.00 10.00           C\nATOM      2  C4'  DA A   1       0.000   0.000   0.000  1.00 10.00           C\nATOM      3  C3'  DA A   1       1.000   0.000   0.000  1.00 10.00           C\nEND\n".to_vec(),
        Some("sugar-direction.pdb"), &molframe::ReadOptions::new(),
    ).expect("sugar atoms parse");
    let mut mesh = RibbonMesh::default();
    mesh.generate_structure(&source, &AtomSelection::All, &[], 8.0, params())
        .expect("sugar backbone defines a direction");
    assert_eq!(mesh.vertices.len(), 3);
    assert!((mesh.vertices[0].position[0] - 0.4).abs() < 1.0e-6);
    let recipe: &[u32] = bytemuck::cast_slice(mesh.deformation_bytes());
    assert_eq!(&recipe[..4], &[0, 1, 2, 1]);
    assert_eq!(recipe[7], 1);
}
