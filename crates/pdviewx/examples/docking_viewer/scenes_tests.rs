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
