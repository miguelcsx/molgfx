use super::Scene;
use crate::{
    Representation, RepresentationKind, RepresentationTarget, SegmentStyle, SegmentStyleTable,
    SegmentationStyle, SegmentedVolume,
};
use molgfx_math::{Mat4, Rgba8, Vec3};
use std::sync::Arc;

#[test]
fn scene_tracks_categorical_volume_lifecycle_and_bounds() {
    let mut scene = Scene::new();
    let volume = match SegmentedVolume::new([2, 2, 2], Mat4::IDENTITY, Arc::from([1; 8])) {
        Ok(volume) => volume,
        Err(error) => panic!("categorical volume builds: {error}"),
    };
    let handle = scene.add_segmented_volume(volume.clone());
    assert_eq!(scene.segmentation_content_revision(handle), Some(0));
    assert_eq!(scene.world_aabb(), volume.world_aabb());
    assert!(scene.segmented_volume(handle).is_some());

    let styles = match SegmentStyleTable::new(&[SegmentStyle::new(1, Rgba8::WHITE, 0.75)]) {
        Ok(styles) => styles,
        Err(error) => panic!("styles build: {error}"),
    };
    let representation = match scene.represent(
        handle,
        Representation::segmentation().segmentation_style(SegmentationStyle {
            styles,
            ..SegmentationStyle::default()
        }),
    ) {
        Ok(representation) => representation,
        Err(error) => panic!("segmentation represents: {error}"),
    };
    let Some(representation_value) = scene.representation(representation) else {
        panic!("representation resolves")
    };
    assert_eq!(representation_value.kind, RepresentationKind::Segmentation);
    assert_eq!(
        representation_value.target,
        RepresentationTarget::SegmentedVolume(handle)
    );
    assert_eq!(representation_value.segmentation.styles.styles().len(), 1);
}

#[test]
fn scene_replaces_and_removes_categorical_volume_handles() {
    let mut scene = Scene::new();
    let first =
        match SegmentedVolume::from_spacing([2, 2, 2], Vec3::ZERO, Vec3::ONE, Arc::from([1; 8])) {
            Ok(volume) => volume,
            Err(error) => panic!("first volume builds: {error}"),
        };
    let second =
        match SegmentedVolume::from_spacing([2, 2, 2], Vec3::ONE, Vec3::ONE, Arc::from([2; 8])) {
            Ok(volume) => volume,
            Err(error) => panic!("second volume builds: {error}"),
        };
    let handle = scene.add_segmented_volume(first);
    assert!(scene.replace_segmented_volume(handle, second).is_ok());
    assert_eq!(scene.segmentation_content_revision(handle), Some(1));
    assert!(scene.remove_segmented_volume(handle).is_some());
    assert!(scene.segmented_volume(handle).is_none());
    let replacement = match SegmentedVolume::new([2, 2, 2], Mat4::IDENTITY, Arc::from([0; 8])) {
        Ok(volume) => volume,
        Err(error) => panic!("replacement volume builds: {error}"),
    };
    assert!(scene.replace_segmented_volume(handle, replacement).is_err());
}
