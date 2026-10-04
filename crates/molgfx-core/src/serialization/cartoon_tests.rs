use crate::{
    AtomSelection, RepresentationConfig, RepresentationKind, Scene, SceneDescriptionSources,
};

#[test]
fn authored_cartoon_proportions_survive_cold_scene_restoration() {
    let structure = crate::fixture::structure();
    let mut scene = Scene::from_structure(&structure).expect("source binds");
    let selection = scene.add_selection(AtomSelection::All);
    let recipe = RepresentationConfig::new(RepresentationKind::Cartoon)
        .cartoon_shape(3.0, 2.0)
        .expect("valid shape");
    scene
        .represent(selection, recipe)
        .expect("cartoon attaches");
    let description = scene.describe();
    let restored = Scene::from_description(
        &description,
        SceneDescriptionSources {
            structures: std::slice::from_ref(&structure),
            volumes: &[],
            segmentations: &[],
            atom_properties: &[],
            meshes: &[],
        },
    )
    .expect("cold restore");
    assert_eq!(restored.describe(), description);
    assert_eq!(
        description.representations[0]
            .cartoon_aspect_ratio
            .to_bits(),
        3.0_f32.to_bits()
    );
    assert_eq!(
        description.representations[0]
            .cartoon_arrow_factor
            .to_bits(),
        2.0_f32.to_bits()
    );
    let mut invalid = description;
    invalid.representations[0].cartoon_aspect_ratio = 0.0;
    assert!(
        Scene::from_description(
            &invalid,
            SceneDescriptionSources {
                structures: std::slice::from_ref(&structure),
                volumes: &[],
                segmentations: &[],
                atom_properties: &[],
                meshes: &[]
            }
        )
        .is_err()
    );
}
