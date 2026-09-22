use crate::{
    BoolExpr, Color, ColorExpr, DataSource, ScalarExpr, ScalarPropertyBinding, Scene, StructureId,
    VisualStyle, color, rep,
};
use std::sync::Arc;

fn structure() -> molframe::Structure {
    const PDB: &str =
        "ATOM      1  N   ALA A   1      11.104   6.134  -6.504  1.00  0.00           N\nEND\n";
    let result = molframe::read_bytes(
        PDB.as_bytes().to_vec(),
        Some("property.pdb"),
        &molframe::ReadOptions::new(),
    );
    let Ok((structure, _)) = result else {
        panic!("fixture must parse")
    };
    structure
}

#[test]
fn one_binding_drives_color_and_visual_property_inputs_without_copying() {
    let values: Arc<[f32]> = Arc::from([0.75]);
    let mut scene = Scene::from_structure(&structure()).unwrap_or_else(|error| panic!("{error}"));
    let property = scene
        .bind_property(
            ScalarPropertyBinding::new(
                StructureId::new(1),
                "confidence",
                DataSource::new("confidence-v1"),
                Arc::clone(&values),
            )
            .units("score"),
        )
        .unwrap_or_else(|error| panic!("{error}"));
    assert!(Arc::strong_count(&values) >= 3);
    let colored = rep::spacefill("all").color(color::property(
        property.clone(),
        "viridis",
        [0.0, 1.0],
        Some("score".into()),
        Color::rgb(112, 112, 112),
    ));
    let _ = scene.add(colored).unwrap_or_else(|error| panic!("{error}"));
    let visual = VisualStyle::new(
        ColorExpr::Ramp {
            value: ScalarExpr::property(property).into(),
            palette: "viridis".into(),
            domain: [0.0, 1.0],
            missing: Color::rgb(112, 112, 112),
        },
        1.0,
        BoolExpr::Constant(true),
    );
    let _ = scene
        .add(rep::points("all").visual(visual))
        .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(scene.spec().properties.len(), 1);
}

#[test]
fn invalid_property_rows_leave_the_scene_unchanged() {
    let mut scene = Scene::from_structure(&structure()).unwrap_or_else(|error| panic!("{error}"));
    let before = scene.to_spec();
    let result = scene.bind_property(ScalarPropertyBinding::new(
        StructureId::new(1),
        "bad",
        DataSource::new("bad-v1"),
        Arc::from([1.0_f32, 2.0]),
    ));
    assert!(result.is_err());
    assert_eq!(scene.spec(), &before);
}
