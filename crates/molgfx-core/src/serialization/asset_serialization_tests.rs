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

#[test]
fn exact_secondary_labels_survive_manifest_rehydration_and_invalid_labels_fail() {
    let structure = crate::fixture::structure();
    let mut scene = Scene::from_structure(&structure).unwrap();
    let owner = scene.structures().next().unwrap().0;
    for state in crate::SecondaryStructure::ALL {
        scene
            .apply_secondary_structure(owner, &[(molframe::ResidueIndex::new(0), state)])
            .unwrap();
        let description = scene.describe();
        assert_eq!(
            description.structures[0].secondary_structure[0],
            state.name()
        );
        let sources = || SceneDescriptionSources {
            structures: std::slice::from_ref(&structure),
            volumes: &[],
            segmentations: &[],
            atom_properties: &[],
            meshes: &[],
        };
        let rebuilt = Scene::from_description(&description, sources()).unwrap();
        assert_eq!(rebuilt.describe(), description);
        let mut invalid = description;
        invalid.structures[0].secondary_structure[0] = "helix".into();
        assert!(Scene::from_description(&invalid, sources()).is_err());
    }
}
