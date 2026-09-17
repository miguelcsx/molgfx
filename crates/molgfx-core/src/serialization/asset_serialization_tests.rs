use crate::{DatasetId, Scene, SceneDescriptionSources, StructureAsset};

#[test]
fn manifests_preserve_the_callers_dataset_identity() {
    let structure = crate::fixture::structure();
    let asset = match StructureAsset::new(DatasetId::new(9_001), &structure) {
        Ok(asset) => asset,
        Err(error) => panic!("fixture asset builds: {error}"),
    };
    let mut scene = Scene::from_asset(&asset);
    scene.add_asset(&asset);
    let description = scene.describe();
    assert_eq!(description.structures[0].dataset_id, 9_001);
    assert_eq!(description.structures[1].dataset_id, 9_001);

    let rebuilt = match Scene::from_description(
        &description,
        SceneDescriptionSources {
            structures: std::slice::from_ref(&structure),
            volumes: &[],
            segmentations: &[],
            atom_properties: &[],
            meshes: &[],
        },
    ) {
        Ok(scene) => scene,
        Err(error) => panic!("asset scene rehydrates: {error}"),
    };
    let placements = rebuilt
        .structures()
        .map(|(_, placed)| placed)
        .collect::<Vec<_>>();
    let [first, second] = placements.as_slice() else {
        panic!("two rehydrated placements exist")
    };
    assert_eq!(first.dataset_id(), DatasetId::new(9_001));
    assert_eq!(second.dataset_id(), DatasetId::new(9_001));
    assert!(std::sync::Arc::ptr_eq(&first.atoms, &second.atoms));
    assert!(std::sync::Arc::ptr_eq(&first.hierarchy, &second.hierarchy));
    assert_eq!(rebuilt.describe(), description);
}
