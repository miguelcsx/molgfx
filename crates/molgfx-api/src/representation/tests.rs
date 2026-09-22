use super::*;
use crate::{Scene, rep};

fn structure() -> molframe::Structure {
    const PDB: &str =
        "ATOM      1  N   ALA A   1      11.104   6.134  -6.504  1.00  0.00           N\nEND\n";
    let result = molframe::read_bytes(
        PDB.as_bytes().to_vec(),
        Some("one.pdb"),
        &molframe::ReadOptions::new(),
    );
    let Ok((structure, _)) = result else {
        panic!("fixture must parse")
    };
    structure
}

#[test]
fn every_representation_form_round_trips_through_its_tagged_shape() {
    let specifications = vec![
        rep::cartoon("all").into(),
        rep::ball_and_stick("all").into(),
        rep::spacefill("all").into(),
        rep::licorice("all").into(),
        rep::lines("all").into(),
        rep::points("all").into(),
        rep::surface("all").into(),
        rep::nucleic_acid("all").into(),
        rep::bases("all").into(),
        rep::base_pairs("all").into(),
        rep::glycan("all").into(),
    ];
    for specification in specifications {
        let json = serde_json::to_string(&specification)
            .unwrap_or_else(|error| panic!("specification must serialize: {error}"));
        let decoded: RepresentationSpec = serde_json::from_str(&json)
            .unwrap_or_else(|error| panic!("specification must deserialize: {error}"));
        assert_eq!(decoded, specification);
        assert!(json.contains("\"common\""));
        assert!(json.contains("\"kind\""));
    }
}

#[test]
fn the_superseded_optional_field_wire_shape_is_rejected() {
    let mut scene = Scene::from_structure(&structure())
        .unwrap_or_else(|error| panic!("scene must resolve: {error}"));
    let id = scene
        .add(rep::surface("all"))
        .unwrap_or_else(|error| panic!("surface must resolve: {error}"));
    let mut value = serde_json::to_value(scene.spec())
        .unwrap_or_else(|error| panic!("scene must serialize: {error}"));
    let Some(representations) = value
        .get_mut("representations")
        .and_then(serde_json::Value::as_object_mut)
    else {
        panic!("representations must be an object")
    };
    let _ = representations.insert(
        id.get().to_string(),
        serde_json::json!({
            "structure": 1,
            "target": "all",
            "form": "surface",
            "color": { "kind": "uniform", "color": [255, 255, 255, 255] },
            "opacity": 1.0,
            "probe_radius": 1.4,
            "visible": true
        }),
    );
    let source = serde_json::to_string(&value)
        .unwrap_or_else(|error| panic!("malformed fixture must serialize: {error}"));
    assert!(crate::SceneSpec::from_json(&source).is_err());
}

#[test]
fn a_form_control_only_reaches_its_own_form() {
    let spec: RepresentationSpec = rep::surface("all")
        .kind(SurfaceKind::Gaussian)
        .isolevel(0.75)
        .probe_radius(1.8)
        .into();
    let Ok(json) = serde_json::to_string(&spec) else {
        panic!("surface must serialize")
    };
    assert!(json.contains("\"kind\":\"surface\""), "{json}");
    assert!(json.contains("\"surface\":\"gaussian\""), "{json}");
    assert!(json.contains("\"isolevel\":0.75"), "{json}");

    // A cartoon carries no surface controls at all, in the builder or the wire.
    let cartoon: RepresentationSpec = rep::cartoon("all").width(2.0).into();
    let Ok(json) = serde_json::to_string(&cartoon) else {
        panic!("cartoon must serialize")
    };
    assert!(!json.contains("isolevel"), "{json}");
    assert!(!json.contains("probe_radius"), "{json}");
}
