use super::*;
use crate::fixture;
use molgfx_math::Vec3;

#[test]
fn cloned_assets_share_every_base_column() {
    let source = fixture::structure();
    let asset = match StructureAsset::new(DatasetId::new(42), &source) {
        Ok(asset) => asset,
        Err(error) => panic!("fixture asset rejected: {error}"),
    };
    let clone = asset.clone();
    assert!(asset.shares_storage_with(&clone));
    assert_eq!(
        asset.atoms().coords().slice().as_ptr(),
        source.positions().as_ptr()
    );
    assert_eq!(
        asset.atoms().element().values().as_ptr(),
        clone.atoms().element().values().as_ptr()
    );
    assert_eq!(
        asset.hierarchy().residue_atom_start.as_ptr(),
        clone.hierarchy().residue_atom_start.as_ptr()
    );
}

#[test]
fn placements_only_add_independent_transforms() {
    let source = fixture::structure();
    let asset = match StructureAsset::new(DatasetId::new(7), &source) {
        Ok(asset) => asset,
        Err(error) => panic!("fixture asset rejected: {error}"),
    };
    let first = match asset.place(Mat4::IDENTITY) {
        Ok(placement) => placement,
        Err(error) => panic!("identity placement rejected: {error}"),
    };
    let second = match asset.place(Mat4::from_translation(Vec3::X * 5.0)) {
        Ok(placement) => placement,
        Err(error) => panic!("translated placement rejected: {error}"),
    };
    assert!(first.asset().shares_storage_with(second.asset()));
    assert_ne!(first.model_to_world(), second.model_to_world());
    assert_ne!(first.world_aabb(), second.world_aabb());
}

#[test]
fn a_missing_model_returns_a_typed_error() {
    let source = fixture::structure();
    assert!(matches!(
        StructureAsset::for_model(DatasetId::new(1), &source, ModelIndex::new(99)),
        Err(DatasetError::MissingStructureModel)
    ));
}
