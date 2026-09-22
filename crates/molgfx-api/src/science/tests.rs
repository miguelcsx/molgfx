use crate::{
    Anchor, DataSource, InteractionKind, Scene, ScenePatch, SceneSpec, StructureId, VolumeBinding,
    annotation, density, interaction, measurement, trajectory,
};
use std::sync::Arc;

fn structure() -> molframe::Structure {
    const PDB: &str =
        "ATOM      1  N   ALA A   1      11.104   6.134  -6.504  1.00  0.00           N\nEND\n";
    match molframe::read_bytes(
        PDB.as_bytes().to_vec(),
        Some("one.pdb"),
        &molframe::ReadOptions::new(),
    ) {
        Ok((structure, _)) => structure,
        Err(error) => panic!("fixture must parse: {error:?}"),
    }
}

fn anchor() -> Anchor {
    Anchor::Selection {
        structure: StructureId::new(1),
        selection: "all".into(),
    }
}

fn world() -> Anchor {
    Anchor::World { position: [0.0; 3] }
}

#[test]
fn every_scientific_item_receives_its_own_monotonic_identity() {
    let mut scene = Scene::from_structure(&structure()).unwrap_or_else(|error| panic!("{error}"));
    let volume = scene
        .add(density::volume(DataSource::new("density-hash"), [8, 8, 8]))
        .unwrap_or_else(|error| panic!("{error}"));
    let label = scene
        .add(annotation::label(anchor(), "active site"))
        .unwrap_or_else(|error| panic!("{error}"));
    let distance = scene
        .add(measurement::distance(anchor(), world()))
        .unwrap_or_else(|error| panic!("{error}"));
    let contact = scene
        .add(interaction::explicit(
            InteractionKind::Contact,
            anchor(),
            world(),
        ))
        .unwrap_or_else(|error| panic!("{error}"));
    let trajectory = scene
        .add(trajectory::trajectory(
            StructureId::new(1),
            DataSource::new("trajectory-hash"),
            10,
        ))
        .unwrap_or_else(|error| panic!("{error}"));

    assert_eq!(volume.get(), 1);
    assert_eq!(label.get(), 1);
    assert_eq!(distance.get(), 1);
    assert_eq!(contact.get(), 1);
    assert_eq!(trajectory.get(), 1);
    assert_eq!(scene.spec().volumes.len(), 1);
    assert_eq!(scene.spec().annotations.len(), 1);
    assert_eq!(scene.spec().measurements.len(), 1);
    assert_eq!(scene.spec().scientific_interactions.len(), 1);
    assert_eq!(scene.spec().trajectories.len(), 1);
}

#[test]
fn every_exposed_scientific_capability_reaches_the_renderer() {
    let mut scene = Scene::from_structure(&structure()).unwrap_or_else(|error| panic!("{error}"));
    let source = DataSource::new("density-hash");
    let _ = scene
        .add(density::volume(source.clone(), [2, 2, 2]))
        .unwrap_or_else(|error| panic!("{error}"));
    let values: Arc<[f32]> = (0_u16..8).map(f32::from).collect();
    scene
        .bind_volume(VolumeBinding::new(source, [2, 2, 2], values))
        .unwrap_or_else(|error| panic!("{error}"));
    let _ = scene
        .add(annotation::label(anchor(), "active site"))
        .unwrap_or_else(|error| panic!("{error}"));
    let _ = scene
        .add(measurement::distance(anchor(), world()))
        .unwrap_or_else(|error| panic!("{error}"));
    let _ = scene
        .add(interaction::explicit(
            InteractionKind::HydrogenBond,
            anchor(),
            world(),
        ))
        .unwrap_or_else(|error| panic!("{error}"));

    let handles = scene.scientific_handles();
    assert_eq!(handles.volumes, 1, "a bound density grid is uploaded");
    assert_eq!(handles.labels, 1, "a label becomes an annotation");
    assert_eq!(handles.measurements, 1, "a distance becomes a measurement");
    assert_eq!(handles.interactions, 1, "an explicit edge becomes an edge");
}

#[test]
fn a_volume_without_a_runtime_binding_stays_unresolved() {
    let mut scene = Scene::from_structure(&structure()).unwrap_or_else(|error| panic!("{error}"));
    let _ = scene
        .add(density::volume(DataSource::new("unbound-hash"), [2, 2, 2]))
        .unwrap_or_else(|error| panic!("{error}"));

    assert_eq!(scene.scientific_handles().volumes, 0);
    assert_eq!(scene.spec().volumes.len(), 1);
}

#[test]
fn detected_interactions_are_not_exposed() {
    let rejected = serde_json::from_str::<crate::ScientificInteractionSpec>(
        r#"{"mode":"detected","kind":"hydrogen_bond","structure":1,"selection":"all","cutoff":4.0}"#,
    );
    assert!(rejected.is_err(), "detected interactions must not decode");
    let explicit = interaction::explicit(
        InteractionKind::Contact,
        world(),
        Anchor::World {
            position: [1.0, 0.0, 0.0],
        },
    );
    let encoded = serde_json::to_string(&explicit).unwrap_or_else(|error| panic!("{error}"));
    assert!(encoded.contains("explicit"), "{encoded}");
}

#[test]
fn scientific_specs_round_trip_without_bulk_payloads() {
    let mut scene = Scene::from_structure(&structure()).unwrap_or_else(|error| panic!("{error}"));
    let _ = scene
        .add(density::volume(
            DataSource::new("grid-hash").uri("https://example.invalid/grid.bcif"),
            [2, 3, 4],
        ))
        .unwrap_or_else(|error| panic!("{error}"));
    let json = scene
        .spec()
        .to_json()
        .unwrap_or_else(|error| panic!("{error}"));
    let decoded = SceneSpec::from_json(&json).unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(decoded, *scene.spec());
    assert!(!json.contains("grid_values"));
}

#[test]
fn invalid_scientific_items_leave_the_scene_unchanged() {
    let mut scene = Scene::from_structure(&structure()).unwrap_or_else(|error| panic!("{error}"));
    let before = scene.to_spec();
    let result = scene.add(density::volume(DataSource::new(""), [0, 2, 2]));
    assert!(result.is_err());
    assert_eq!(scene.spec(), &before);
}

#[test]
fn scientific_additions_have_exact_inverse_patches() {
    let scene = Scene::from_structure(&structure()).unwrap_or_else(|error| panic!("{error}"));
    let base = scene.to_spec();
    let volume = density::volume(DataSource::new("density-hash"), [2, 2, 2]);
    let mut applied_scene = Scene::from_spec(base.clone(), {
        let mut structures = std::collections::BTreeMap::new();
        let _ = structures.insert(StructureId::new(1), structure());
        structures
    })
    .unwrap_or_else(|error| panic!("{error}"));
    let id = applied_scene
        .add(volume)
        .unwrap_or_else(|error| panic!("{error}"));
    let patch = ScenePatch {
        base_revision: base.revision,
        operations: vec![crate::PatchOperation::AddVolume {
            id,
            volume: applied_scene
                .spec()
                .volumes
                .get(&id)
                .cloned()
                .unwrap_or_else(|| panic!("volume exists")),
        }],
    };
    let inverse = patch
        .inverse(&base)
        .unwrap_or_else(|error| panic!("{error}"));
    let restored = base
        .patched(&patch)
        .and_then(|value| value.patched(&inverse))
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(restored.volumes, base.volumes);
}
