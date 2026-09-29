use super::tests::structure;
use super::*;
use crate::{PickKind, PickResult, ResolvedPick};

#[test]
fn atom_picks_resolve_to_stable_source_metadata() {
    let scene = Scene::from_structure(&structure()).unwrap_or_else(|error| panic!("{error}"));
    let Some((_, placed)) = scene.resolved.structures().next() else {
        panic!("resolved structure exists")
    };
    let pick = PickResult {
        kind: PickKind::Atom,
        dataset: Some(placed.dataset_id().get()),
        chunk: Some(0),
        row: Some(0),
        volume_label: None,
    };
    let resolved = scene
        .resolve_pick(&pick)
        .unwrap_or_else(|error| panic!("pick resolves: {error}"));
    let ResolvedPick::Atom(atom) = resolved else {
        panic!("atom pick remains an atom")
    };
    assert_eq!(atom.model, Some(0));
    assert_eq!(atom.entity, Some(0));
    assert_eq!(atom.atom_name.as_deref(), Some("N"));
    assert_eq!(atom.element, Some(7));
    assert_eq!(atom.occupancy, Some(1.0));
    assert_eq!(atom.b_factor, Some(0.0));
    assert!(atom.label.contains("N ALA1 A"));
}

#[test]
fn bond_picks_resolve_to_topology_endpoints_and_order() {
    const PDB: &str = "ATOM      1  C   GLY A   1       0.000   0.000   0.000  1.00  0.00           C\nATOM      2  N   GLY A   1       1.330   0.000   0.000  1.00  0.00           N\nCONECT    1    2\nEND\n";
    let (source, _) = molframe::read_bytes(
        PDB.as_bytes().to_vec(),
        Some("bond.pdb"),
        &molframe::ReadOptions::new(),
    )
    .unwrap_or_else(|error| panic!("bond fixture reads: {error:?}"));
    let scene = Scene::from_structure(&source).unwrap_or_else(|error| panic!("{error}"));
    let Some((_, placed)) = scene.resolved.structures().next() else {
        panic!("resolved structure exists")
    };
    let pick = PickResult {
        kind: PickKind::Bond,
        dataset: Some(placed.dataset_id().get()),
        chunk: Some(0),
        row: Some(0),
        volume_label: None,
    };

    let resolved = scene
        .resolve_pick(&pick)
        .unwrap_or_else(|error| panic!("bond pick resolves: {error}"));
    let ResolvedPick::Bond(bond) = resolved else {
        panic!("bond pick resolves to a bond")
    };
    assert_eq!([bond.atom_a, bond.atom_b], [0, 1]);
    // PDB CONECT records identify connectivity but do not carry an order.
    assert_eq!(bond.order.as_deref(), Some("unknown"));
    assert!(!bond.aromatic);
    assert!(!bond.metal);
    assert_eq!(bond.weight, None);
}

#[test]
fn resolved_picks_serialize_with_a_pick_discriminator_and_round_trip() {
    let scene = Scene::from_structure(&structure()).unwrap_or_else(|error| panic!("{error}"));
    let Some((_, placed)) = scene.resolved.structures().next() else {
        panic!("resolved structure exists")
    };
    let atom = scene
        .resolve_pick(&PickResult {
            kind: PickKind::Atom,
            dataset: Some(placed.dataset_id().get()),
            chunk: Some(0),
            row: Some(0),
            volume_label: None,
        })
        .unwrap_or_else(|error| panic!("{error}"));
    let json = serde_json::to_value(&atom).unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(json["pick"], "atom");
    assert_eq!(json["atom_name"], "N");
    let back: ResolvedPick = serde_json::from_value(json).unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(back, atom);

    let label = scene
        .resolve_pick(&PickResult {
            kind: PickKind::Label,
            dataset: None,
            chunk: None,
            row: Some(3),
            volume_label: None,
        })
        .unwrap_or_else(|error| panic!("{error}"));
    let json = serde_json::to_value(&label).unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(json["pick"], "non_atom");
    assert_eq!(json["kind"], "label");
    let back: ResolvedPick = serde_json::from_value(json).unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(back, label);
}

#[test]
fn the_superseded_externally_tagged_pick_shape_is_rejected() {
    let result = serde_json::from_str::<ResolvedPick>(r#"{"Atom":{"atom_index":0}}"#);
    assert!(result.is_err());
}

#[test]
fn a_label_pick_resolves_to_the_annotation_the_scene_owns() {
    let mut scene = Scene::from_structure(&structure()).unwrap_or_else(|error| panic!("{error}"));
    let anchor = crate::Anchor::World {
        position: [1.0, 2.0, 3.0],
    };
    let annotation = scene
        .add(crate::annotation::label(anchor, "site"))
        .unwrap_or_else(|error| panic!("label adds: {error}"));

    // The renderer packs the annotation's storage row into the pick, so the
    // test resolves against the row the scene actually assigned.
    let row = scene
        .overlay
        .labels
        .iter()
        .find(|(id, _)| *id == annotation)
        .map(|(_, handle)| handle.row())
        .unwrap_or_else(|| panic!("the label is lowered"));
    let resolved = scene
        .resolve_pick(&PickResult {
            kind: PickKind::Label,
            dataset: None,
            chunk: None,
            row: Some(u64::from(row)),
            volume_label: None,
        })
        .unwrap_or_else(|error| panic!("{error}"));
    let ResolvedPick::Label(label) = &resolved else {
        panic!("a label pick resolves to its annotation, got {resolved:?}")
    };
    assert_eq!(label.annotation, annotation);
    assert_eq!(label.text, "site");
    let json = serde_json::to_value(&resolved).unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(json["pick"], "label");
    assert_eq!(json["text"], "site");
    let back: ResolvedPick = serde_json::from_value(json).unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(back, resolved);
}

#[test]
fn a_measurement_pick_resolves_to_its_kind_and_arity() {
    let mut scene = Scene::from_structure(&structure()).unwrap_or_else(|error| panic!("{error}"));
    let first = crate::Anchor::World {
        position: [0.0, 0.0, 0.0],
    };
    let second = crate::Anchor::World {
        position: [2.0, 0.0, 0.0],
    };
    let measurement = scene
        .add(crate::measurement::distance(
            crate::Anchor::World {
                position: [0.0, 0.0, 0.0],
            },
            crate::Anchor::World {
                position: [2.0, 0.0, 0.0],
            },
        ))
        .unwrap_or_else(|error| panic!("measurement adds: {error}"));

    // Annotations and measurements share one label storage table and are told
    // apart by the entity kind the renderer packs, so a measurement pick
    // resolves to the measurement at its row even with a label beside it.
    let _ = scene
        .add(crate::annotation::label(first, "first"))
        .unwrap_or_else(|error| panic!("label adds: {error}"));
    let row = scene
        .overlay
        .measurements
        .iter()
        .find(|(id, _)| *id == measurement)
        .map(|(_, handle)| handle.row())
        .unwrap_or_else(|| panic!("the measurement is lowered"));
    let resolved = scene
        .resolve_pick(&PickResult {
            kind: PickKind::Measurement,
            dataset: None,
            chunk: None,
            row: Some(u64::from(row)),
            volume_label: None,
        })
        .unwrap_or_else(|error| panic!("{error}"));
    let ResolvedPick::Measurement(picked) = &resolved else {
        panic!("a measurement pick resolves to its own kind, got {resolved:?}")
    };
    assert_eq!(picked.measurement, measurement);
    assert_eq!(picked.kind, "distance");
    assert_eq!(picked.arity, 2);
}

#[test]
fn a_volume_segment_pick_resolves_to_its_volume_and_label() {
    let mut scene = Scene::from_structure(&structure()).unwrap_or_else(|error| panic!("{error}"));
    let volume = scene
        .add(crate::density::volume(
            crate::overlay::DataSource::new("density-sha256"),
            [2, 2, 2],
        ))
        .unwrap_or_else(|error| panic!("volume adds: {error}"));

    let resolved = scene
        .resolve_pick(&PickResult {
            kind: PickKind::VolumeSegment,
            dataset: None,
            chunk: None,
            row: None,
            volume_label: Some(42),
        })
        .unwrap_or_else(|error| panic!("{error}"));
    let ResolvedPick::VolumeSegment(segment) = &resolved else {
        panic!("a volume-segment pick resolves to its volume, got {resolved:?}")
    };
    assert_eq!(segment.volume, volume);
    assert_eq!(segment.volume_label, 42);
    let json = serde_json::to_value(&resolved).unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(json["pick"], "volume_segment");
    assert_eq!(json["volume_label"], 42);
}
