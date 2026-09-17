use super::*;
use crate::{
    Annotation, AnnotationAnchor, Guide, GuideStyle, InteractionAnchor, InteractionGeometry,
    InteractionKind, Material, Measurement, Mesh, MeshVertex,
};
use molgfx_math::{Rgba8, Vec3};

fn atom(owner: crate::StructureHandle, index: u32) -> EntityRef {
    EntityRef {
        structure: owner,
        kind: EntityKind::Atom,
        index,
    }
}

#[test]
fn atoms_and_bonds_resolve_directly_to_pdbiox_source_rows() {
    let structure = crate::fixture::structure();
    let scene = Scene::from_structure(&structure).unwrap_or_else(|error| panic!("{error}"));
    let Some((owner, _)) = scene.structures().next() else {
        panic!("fixture structure exists")
    };
    let Some(atom_provenance) = scene.provenance(atom(owner, 0)) else {
        panic!("atom provenance resolves")
    };
    let ProvenanceDetail::Atom(source) = atom_provenance.detail else {
        panic!("atom source kind is retained")
    };
    assert_eq!(source.name(), Some("N"));
    let bond = EntityRef {
        structure: owner,
        kind: EntityKind::Bond,
        index: 0,
    };
    assert!(matches!(
        scene.provenance(bond).map(|value| value.detail),
        Some(ProvenanceDetail::Bond(_))
    ));
}

#[test]
fn interactions_annotations_and_measurements_keep_their_source_objects() {
    let structure = crate::fixture::structure();
    let mut scene = Scene::from_structure(&structure).unwrap_or_else(|error| panic!("{error}"));
    let Some((owner, _)) = scene.structures().next() else {
        panic!("fixture structure exists")
    };
    let start = InteractionAnchor::entity(Vec3::ZERO, atom(owner, 0))
        .unwrap_or_else(|error| panic!("{error}"));
    let end = InteractionAnchor::entity(Vec3::X, atom(owner, 1))
        .unwrap_or_else(|error| panic!("{error}"));
    let geometry = InteractionGeometry::new(1.0, None).unwrap_or_else(|error| panic!("{error}"));
    let edge = InteractionEdge::new(
        owner,
        start,
        end,
        InteractionKind::HydrogenBond,
        geometry,
        "pdbiox:test",
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let edge = scene
        .add_interaction(edge)
        .unwrap_or_else(|error| panic!("{error}"));
    let edge_entity = EntityRef {
        structure: owner,
        kind: EntityKind::Edge,
        index: Scene::interaction_row(edge),
    };
    let Some(ProvenanceDetail::Interaction(edge)) =
        scene.provenance(edge_entity).map(|value| value.detail)
    else {
        panic!("interaction provenance resolves")
    };
    assert_eq!(edge.provenance(), "pdbiox:test");

    let anchor = AnnotationAnchor::entity(Vec3::ZERO, atom(owner, 0))
        .unwrap_or_else(|error| panic!("{error}"));
    let note = Annotation::note(owner, anchor, "site").unwrap_or_else(|error| panic!("{error}"));
    let note = scene
        .add_annotation(note)
        .unwrap_or_else(|error| panic!("{error}"));
    let label_entity = EntityRef {
        structure: owner,
        kind: EntityKind::Label,
        index: Scene::annotation_row(note),
    };
    assert!(matches!(
        scene.provenance(label_entity).map(|value| value.detail),
        Some(ProvenanceDetail::Annotation(_))
    ));

    let measurement = Measurement::distance(owner, [anchor, anchor], 1.0, "caller:test")
        .unwrap_or_else(|error| panic!("{error}"));
    let measurement = scene
        .add_measurement(measurement)
        .unwrap_or_else(|error| panic!("{error}"));
    let measurement_entity = EntityRef {
        structure: owner,
        kind: EntityKind::Label,
        index: Scene::measurement_row(measurement),
    };
    assert!(matches!(
        scene
            .provenance(measurement_entity)
            .map(|value| value.detail),
        Some(ProvenanceDetail::Measurement(_))
    ));
}

#[test]
fn guides_and_interactions_with_the_same_row_keep_distinct_provenance() {
    let structure = crate::fixture::structure();
    let mut scene = Scene::from_structure(&structure).unwrap_or_else(|error| panic!("{error}"));
    let Some((owner, _)) = scene.structures().next() else {
        panic!("fixture structure exists")
    };
    let start = InteractionAnchor::entity(Vec3::ZERO, atom(owner, 0))
        .unwrap_or_else(|error| panic!("{error}"));
    let end = InteractionAnchor::entity(Vec3::X, atom(owner, 1))
        .unwrap_or_else(|error| panic!("{error}"));
    let geometry = InteractionGeometry::new(1.0, None).unwrap_or_else(|error| panic!("{error}"));
    let interaction = InteractionEdge::new(
        owner,
        start,
        end,
        InteractionKind::HydrogenBond,
        geometry,
        "pdbiox:test",
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let interaction = scene
        .add_interaction(interaction)
        .unwrap_or_else(|error| panic!("{error}"));
    let guide = Guide::new(owner, Vec3::ZERO, Vec3::Y, GuideStyle::default())
        .unwrap_or_else(|error| panic!("{error}"));
    let guide = scene
        .add_guide(guide)
        .unwrap_or_else(|error| panic!("{error}"));

    assert_eq!(Scene::interaction_row(interaction), Scene::guide_row(guide));
    let interaction_entity = EntityRef {
        structure: owner,
        kind: EntityKind::Edge,
        index: Scene::interaction_row(interaction),
    };
    let guide_entity = EntityRef {
        structure: owner,
        kind: EntityKind::Guide,
        index: Scene::guide_row(guide),
    };
    assert!(matches!(
        scene
            .provenance(interaction_entity)
            .map(|value| value.detail),
        Some(ProvenanceDetail::Interaction(_))
    ));
    assert!(matches!(
        scene.provenance(guide_entity).map(|value| value.detail),
        Some(ProvenanceDetail::Guide(_))
    ));
}

#[test]
fn meshes_resolve_to_their_caller_geometry() {
    let structure = crate::fixture::structure();
    let mut scene = Scene::from_structure(&structure).unwrap_or_else(|error| panic!("{error}"));
    let Some((owner, _)) = scene.structures().next() else {
        panic!("fixture structure exists")
    };
    let vertex = |position| MeshVertex {
        position,
        normal: Vec3::Z,
        color: Rgba8::WHITE,
    };
    let mesh = Mesh::new(
        owner,
        vec![vertex(Vec3::ZERO), vertex(Vec3::X), vertex(Vec3::Y)],
        vec![0, 1, 2],
        Material::default(),
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let handle = scene
        .add_mesh(mesh)
        .unwrap_or_else(|error| panic!("{error}"));
    let entity = EntityRef {
        structure: owner,
        kind: EntityKind::Mesh,
        index: Scene::mesh_row(handle),
    };
    assert!(matches!(
        scene.provenance(entity).map(|value| value.detail),
        Some(ProvenanceDetail::Mesh(_))
    ));
}
