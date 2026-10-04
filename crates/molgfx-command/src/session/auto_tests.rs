//! Session coverage for the `auto` preset and the `volume` declaration.

use super::tests::{fail, operations, run, scene};
use crate::{Command, ErrorKind, Session};

#[test]
fn auto_adds_the_size_appropriate_default_layers_and_registers_them() {
    let mut scene = scene();
    let mut session = Session::new(&scene);
    let outcome = run(&mut session, &mut scene, "auto");
    assert!(
        operations(&outcome)
            .iter()
            .all(|operation| *operation == "add"),
        "{:?}",
        operations(&outcome)
    );
    assert!(!scene.spec().representations.is_empty());
    let layers = session.spec().layers.len();
    assert_eq!(layers, scene.spec().representations.len());
    // The added layers are ordinary layers: a later statement can hide one.
    let name = session
        .spec()
        .layers
        .keys()
        .next()
        .map_or_else(|| panic!("a layer was registered"), ToString::to_string);
    run(&mut session, &mut scene, &format!("hide @{name}"));
}

#[test]
fn auto_round_trips_through_its_canonical_text() {
    let command = Command::Auto { structure: None };
    assert_eq!(command.to_string(), "auto");
    let parsed = crate::Program::parse("auto")
        .unwrap_or_else(|errors| panic!("{errors:?}"))
        .statements()
        .first()
        .map(|statement| statement.command.clone());
    let Some(Command::Auto { structure }) = parsed else {
        panic!("auto parses back to its own command")
    };
    assert!(structure.is_none());
}

#[test]
fn auto_rejects_a_second_structure_name() {
    let mut scene = scene();
    let mut session = Session::new(&scene);
    let error = fail(&mut session, &mut scene, "auto one two");
    assert_eq!(error.kind, ErrorKind::Syntax, "{error}");
}

#[test]
fn volume_declares_a_density_grid_and_undo_removes_it() {
    let mut scene = scene();
    let mut session = Session::new(&scene);
    let command = r#"volume {"source":{"content_hash":"density-sha256","uri":null,"format":null},"dimensions":[4,4,4],"voxel_to_world":[1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1],"presentations":[{"kind":"isosurface","isovalue":1.5,"color":[49,104,142,255],"opacity":1.0,"style":{"kind":"solid"}}],"region":null}"#;
    let outcome = run(&mut session, &mut scene, command);
    assert_eq!(scene.spec().volumes.len(), 1);
    let volume = scene
        .spec()
        .volumes
        .values()
        .next()
        .unwrap_or_else(|| panic!("one volume"));
    assert_eq!(volume.dimensions, [4, 4, 4]);
    assert_eq!(volume.isovalue(), Some(1.5));
    assert_eq!(
        outcome.patch.as_ref().map(|patch| patch.operations.len()),
        Some(1)
    );
    run(&mut session, &mut scene, "undo");
    assert!(scene.spec().volumes.is_empty());
}

#[test]
fn volume_rejects_a_malformed_specification() {
    let mut scene = scene();
    let mut session = Session::new(&scene);
    let error = fail(&mut session, &mut scene, "volume {}");
    assert_eq!(error.kind, ErrorKind::Syntax, "{error}");
    let error = fail(&mut session, &mut scene, "volume");
    assert_eq!(error.kind, ErrorKind::Syntax, "{error}");
}
#[test]
fn assembly_unit_cell_reaches_guide_geometry_and_undo_removes_it() {
    let mut scene = scene();
    let mut session = Session::new(&scene);
    let command = r#"assembly {"structures":[1],"instances":[],"unit_cell":{"lengths":[10.0,11.0,12.0],"angles_degrees":[90.0,90.0,90.0],"origin":[0.0,0.0,0.0]}}"#;
    run(&mut session, &mut scene, command);
    assert_eq!(scene.overlay_handles().unit_cell_guides, 12);
    assert!(scene.spec().assembly.is_some());

    run(&mut session, &mut scene, "undo");
    assert_eq!(scene.overlay_handles().unit_cell_guides, 0);
    assert!(scene.spec().assembly.is_none());
}
#[test]
fn removing_an_assembly_is_undoable_and_redoable() {
    let mut scene = scene();
    let mut session = Session::new(&scene);
    let command = r#"assembly {"structures":[1],"instances":[],"unit_cell":{"lengths":[10.0,11.0,12.0],"angles_degrees":[80.0,95.0,105.0],"origin":[1.0,2.0,3.0]}}"#;
    run(&mut session, &mut scene, command);
    let assembly = scene.spec().assembly.clone();
    run(&mut session, &mut scene, "assembly null");
    assert!(scene.spec().assembly.is_none());
    assert_eq!(scene.overlay_handles().unit_cell_guides, 0);
    run(&mut session, &mut scene, "undo");
    assert_eq!(scene.spec().assembly, assembly);
    assert_eq!(scene.overlay_handles().unit_cell_guides, 12);
    run(&mut session, &mut scene, "redo");
    assert!(scene.spec().assembly.is_none());
    assert_eq!(scene.overlay_handles().unit_cell_guides, 0);
}
#[test]
fn plane_reaches_guide_geometry_and_undo_removes_it() {
    let mut scene = scene();
    let mut session = Session::new(&scene);
    let command = r#"plane {"structure":1,"center":[0.0,0.0,0.0],"normal":[0.0,0.0,1.0],"tangent":[1.0,0.0,0.0],"size":[4.0,6.0],"color":[226,232,240,255]}"#;
    run(&mut session, &mut scene, command);
    assert_eq!(scene.overlay_handles().plane_guides, 4);
    assert_eq!(scene.spec().planes.len(), 1);

    run(&mut session, &mut scene, "undo");
    assert_eq!(scene.overlay_handles().plane_guides, 0);
    assert!(scene.spec().planes.is_empty());
}
