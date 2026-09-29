use crate::{Scene, from_mvsj, from_mvsx, rep, to_mvsj, to_mvsx};

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
fn mvsj_and_mvsx_preserve_the_compatible_scene_subset() {
    let mut scene = Scene::from_structure(&structure()).unwrap_or_else(|error| panic!("{error}"));
    let _ = scene
        .add(rep::spacefill("all").opacity(0.5))
        .unwrap_or_else(|error| panic!("{error}"));
    let json = to_mvsj(scene.spec()).unwrap_or_else(|error| panic!("{error}"));
    let json_import = from_mvsj(&json).unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(json_import.scene.representations.len(), 1);
    assert!(json_import.diagnostics.is_empty());

    let archive = to_mvsx(scene.spec()).unwrap_or_else(|error| panic!("{error}"));
    let archive_import = from_mvsx(&archive).unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(archive_import.scene.representations.len(), 1);
}

#[test]
fn unsupported_molviewspec_nodes_are_reported() {
    let imported = from_mvsj(
        r#"{"metadata":{"version":1},"root":{"kind":"root","children":[{"kind":"transition","params":{"duration_ms":250}}]}}"#,
    )
    .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(imported.diagnostics.len(), 1);
    assert_eq!(imported.diagnostics[0].code.as_ref(), "unsupported_node");
}

#[test]
fn unknown_molviewspec_nodes_are_distinguished_from_unsupported_v1_nodes() {
    let imported = from_mvsj(
        r#"{"metadata":{"version":1},"root":{"kind":"root","children":[{"kind":"future_node"}]}}"#,
    )
    .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(imported.diagnostics.len(), 1);
    assert_eq!(imported.diagnostics[0].code.as_ref(), "unknown_node");
}

#[test]
fn mvs_semantic_ids_focus_camera_and_annotations_survive_import() {
    let imported = from_mvsj(
        r#"{"metadata":{"version":1},"root":{"kind":"root","children":[{"kind":"download","params":{"url":"model.bcif"},"children":[{"kind":"parse","params":{"format":"bcif"},"children":[{"kind":"structure","params":{"molgfx_structure_id":41,"molgfx_content_hash":"abc"},"children":[{"kind":"component","params":{"selector":"protein"},"children":[{"kind":"representation","params":{"type":"cartoon","molgfx_id":73}}]}]}]}]},{"kind":"focus","params":{"selector":"ligand"}},{"kind":"camera","params":{"position":[1,2,3]}},{"kind":"label","params":{"text":"active site"}}]}}"#,
    )
    .unwrap_or_else(|error| panic!("{error}"));
    assert!(
        imported
            .scene
            .structures
            .contains_key(&crate::StructureId::new(41))
    );
    assert!(
        imported
            .scene
            .representations
            .contains_key(&crate::RepresentationId::new(73))
    );
    assert_eq!(
        imported.scene.focus.as_ref().map(crate::Selection::source),
        Some("ligand")
    );
    assert!(
        imported
            .scene
            .extensions
            .contains_key("org.molgfx.mvs.camera")
    );
    assert!(
        imported
            .scene
            .extensions
            .contains_key("org.molgfx.mvs.annotations")
    );
}

#[test]
fn official_mvs_camera_parameters_map_to_typed_scene_camera() {
    let imported = from_mvsj(
        r#"{"metadata":{"version":1},"root":{"kind":"root","children":[{"kind":"camera","params":{"position":[1,2,8],"target":[1,2,3],"up":[0,1,0],"near":0.25}}]}}"#,
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let Some(camera) = imported.scene.camera else {
        panic!("camera must be imported")
    };
    assert_eq!(camera.eye, molgfx_math::Vec3::new(1.0, 2.0, 8.0));
    assert_eq!(camera.target, molgfx_math::Vec3::new(1.0, 2.0, 3.0));
    assert_eq!(camera.up, molgfx_math::Vec3::Y);
    let molgfx_math::Projection::Perspective { near, .. } = camera.projection else {
        panic!("MolViewSpec cameras use perspective projection")
    };
    assert!((near - 0.25).abs() < f32::EPSILON);
}

#[test]
fn mvs_export_places_each_representation_under_only_its_structure() {
    let source = structure();
    let mut scene = Scene::from_structure(&source).unwrap_or_else(|error| panic!("{error}"));
    let second = scene
        .add_structure(&source)
        .unwrap_or_else(|error| panic!("{error}"));
    let _ = scene
        .add(rep::cartoon("all").structure(crate::StructureId::new(1)))
        .unwrap_or_else(|error| panic!("{error}"));
    let _ = scene
        .add(rep::spacefill("all").structure(second))
        .unwrap_or_else(|error| panic!("{error}"));
    let json = to_mvsj(scene.spec()).unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(json.matches(r#""kind":"representation""#).count(), 2);
}
