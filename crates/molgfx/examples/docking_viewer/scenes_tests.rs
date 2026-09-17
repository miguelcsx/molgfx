use super::*;
use crate::catalog::RepresentationChoice;

#[test]
fn comparison_switches_every_structure_representation() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("benchmarks/scenes/3PTB.cif");
    let Some(path) = path.to_str() else {
        panic!("fixture path is UTF-8")
    };
    let paths = [path.to_owned(), path.to_owned()];
    let (mut app, _) = match build_compare_scene(&paths) {
        Ok(value) => value,
        Err(error) => panic!("comparison builds: {error}"),
    };
    assert_eq!(app.representations.len(), 2);
    app.set_representation(RepresentationChoice::Kind(RepresentationKind::Points));
    for handle in &app.representations {
        let Some(representation) = app.scene.representation(*handle) else {
            panic!("comparison representation resolves")
        };
        assert_eq!(representation.kind, RepresentationKind::Points);
    }
}

#[test]
fn protein_viewer_keeps_a_visible_mode_for_specialized_choices() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("benchmarks/scenes/3PTB.cif");
    let Some(path) = path.to_str() else {
        panic!("fixture path is UTF-8")
    };
    let (mut app, _) = match build_plain_scene(path) {
        Ok(value) => value,
        Err(error) => panic!("plain viewer builds: {error}"),
    };
    let twister = RepresentationChoice::Kind(RepresentationKind::Twister);
    let paper_chain = RepresentationChoice::Kind(RepresentationKind::PaperChain);
    assert!(!app.choice_available(twister));
    assert!(!app.choice_available(paper_chain));
    let handle = app.representations[0];
    let before = app.scene.representation(handle).map(|value| value.kind);
    assert!(!app.set_representation(twister));
    assert_eq!(
        app.scene.representation(handle).map(|value| value.kind),
        before
    );
}

#[test]
fn cycling_skips_choices_that_have_no_geometry_for_the_structure() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("benchmarks/scenes/3PTB.cif");
    let Some(path) = path.to_str() else {
        panic!("fixture path is UTF-8")
    };
    let (mut app, _) = match build_plain_scene(path) {
        Ok(value) => value,
        Err(error) => panic!("plain viewer builds: {error}"),
    };
    app.representation_index = RepresentationChoice::Kind(RepresentationKind::Surface).index();
    assert_eq!(
        app.cycled_choice(crate::catalog::CycleDirection::Next),
        Some(RepresentationChoice::Kind(RepresentationKind::Trace))
    );
}
