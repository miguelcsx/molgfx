use super::*;
use crate::{DatasetId, SecondaryStructure, StructureAsset};
use std::sync::Arc;

fn asset() -> StructureAsset {
    match StructureAsset::new(DatasetId::new(77), &crate::fixture::structure()) {
        Ok(asset) => asset,
        Err(error) => panic!("fixture asset builds: {error}"),
    }
}

#[test]
fn placements_share_atom_hierarchy_and_bvh_allocations() {
    let asset = asset();
    let mut scene = Scene::from_asset(&asset);
    let Some((first_handle, _)) = scene.structures().next() else {
        panic!("first placement exists")
    };
    let second_handle = scene.add_asset(&asset);
    let Some(first) = scene.structure(first_handle) else {
        panic!("first placement resolves")
    };
    let Some(second) = scene.structure(second_handle) else {
        panic!("second placement resolves")
    };

    assert_eq!(first.dataset_id(), DatasetId::new(77));
    assert_eq!(second.dataset_id(), DatasetId::new(77));
    assert!(first.asset().shares_storage_with(second.asset()));
    assert!(Arc::ptr_eq(&first.atoms, &second.atoms));
    assert!(Arc::ptr_eq(&first.hierarchy, &second.hierarchy));
    assert_eq!(Arc::strong_count(&first.atoms), 3);
    assert_eq!(Arc::strong_count(&first.hierarchy), 3);
    assert!(!first.spatial_bvh_is_ready());
    let first_bvh = std::ptr::from_ref(
        first
            .spatial_bvh()
            .unwrap_or_else(|error| panic!("{error}")),
    );
    assert!(second.spatial_bvh_is_ready());
    let second_bvh = std::ptr::from_ref(
        second
            .spatial_bvh()
            .unwrap_or_else(|error| panic!("{error}")),
    );
    assert_eq!(first_bvh, second_bvh);
}

#[test]
fn mutable_secondary_structure_remains_placement_local() {
    let asset = asset();
    let mut scene = Scene::from_asset(&asset);
    let Some((first, _)) = scene.structures().next() else {
        panic!("first placement exists")
    };
    let second = scene.add_asset(&asset);
    if let Err(error) = scene.apply_secondary_structure(
        first,
        &[(molframe::ResidueIndex::new(0), SecondaryStructure::Helix)],
    ) {
        panic!("secondary structure applies: {error}");
    }
    let Some(first) = scene.structure(first) else {
        panic!("first placement resolves")
    };
    let Some(second) = scene.structure(second) else {
        panic!("second placement resolves")
    };
    assert_eq!(
        first.secondary_structure.values()[0],
        SecondaryStructure::Helix
    );
    assert_eq!(
        second.secondary_structure.values()[0],
        SecondaryStructure::Unknown
    );
}

#[test]
fn transitional_structure_constructor_uses_the_legacy_dataset() {
    let scene = match Scene::from_structure(&crate::fixture::structure()) {
        Ok(scene) => scene,
        Err(error) => panic!("legacy scene builds: {error}"),
    };
    let Some((_, placed)) = scene.structures().next() else {
        panic!("legacy placement exists")
    };
    assert_eq!(placed.dataset_id(), DatasetId::LEGACY);
}
