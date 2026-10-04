use super::SegmentationUniforms;
use crate::scene_gpu::segmentation_lookup::{LookupMode, SegmentLookup};
use molgfx_core::{
    ClipPlane, Representation, Scene, SegmentStyle, SegmentStyleTable, SegmentationStyle,
    SegmentedVolume, VolumeRegion, VolumeSlice,
};
use molgfx_math::{Mat4, Rgba8, Vec3};
use std::sync::Arc;

fn representation_and_volume() -> (SegmentedVolume, molgfx_core::Representation) {
    let volume = match SegmentedVolume::new([4, 4, 4], Mat4::IDENTITY, Arc::from([1; 64])) {
        Ok(volume) => volume,
        Err(error) => panic!("segmentation builds: {error}"),
    };
    let mut scene = Scene::new();
    let handle = scene.add_segmented_volume(volume.clone());
    let styles = match SegmentStyleTable::new(&[
        SegmentStyle::new(1, Rgba8::opaque(255, 0, 0), 0.75),
        SegmentStyle::new(2, Rgba8::opaque(0, 255, 0), 0.5),
    ]) {
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
        Ok(handle) => handle,
        Err(error) => panic!("segmentation represents: {error}"),
    };
    let Some(representation) = scene.representation(representation) else {
        panic!("representation resolves")
    };
    (volume, representation.clone())
}

#[test]
fn compact_labels_use_direct_style_lookup() {
    let (_, representation) = representation_and_volume();
    let lookup = SegmentLookup::new(representation.segmentation.styles.styles());
    assert_eq!(lookup.mode(), LookupMode::Direct);
    assert_eq!(lookup.entries().len(), 3);
    assert_eq!(lookup.entries()[1].present, 1);
}

#[test]
fn sparse_labels_use_bounded_hash_style_lookup() {
    let styles = match SegmentStyleTable::new(&[
        SegmentStyle::new(2, Rgba8::WHITE, 1.0),
        SegmentStyle::new(1_000_000, Rgba8::WHITE, 1.0),
        SegmentStyle::new(u32::MAX, Rgba8::WHITE, 1.0),
    ]) {
        Ok(styles) => styles,
        Err(error) => panic!("sparse styles build: {error}"),
    };
    let lookup = SegmentLookup::new(styles.styles());
    assert_eq!(lookup.mode(), LookupMode::Hash);
    assert_eq!(lookup.entries().len(), 8);
    let mut packed = lookup
        .entries()
        .iter()
        .filter(|entry| entry.present != 0)
        .map(|entry| entry.label)
        .collect::<Vec<_>>();
    packed.sort_unstable();
    assert_eq!(packed, vec![2, 1_000_000, u32::MAX]);
}

#[test]
fn categorical_uniforms_pack_crop_slice_clip_and_source_identity() {
    let (volume, mut representation) = representation_and_volume();
    let region = match VolumeRegion::new([1, 0, 1], [4, 3, 4], volume.dimensions()) {
        Ok(region) => region,
        Err(error) => panic!("region builds: {error}"),
    };
    let plane = match ClipPlane::from_point_normal(Vec3::ZERO, Vec3::Y) {
        Ok(plane) => plane,
        Err(error) => panic!("plane builds: {error}"),
    };
    representation.segmentation.region = Some(region);
    representation.segmentation.slice = Some(VolumeSlice::new(plane));
    representation.clipping = match molgfx_core::ClipSet::new(&[plane]) {
        Ok(clipping) => clipping,
        Err(error) => panic!("clipping builds: {error}"),
    };
    let lookup = SegmentLookup::new(representation.segmentation.styles.styles());
    let uniforms = SegmentationUniforms::new(&volume, &representation, 17, &lookup);
    assert_eq!(uniforms.lookup[3], 17);
    assert_eq!(uniforms.crop_minimum, [1, 0, 1, 0]);
    assert_eq!(uniforms.crop_maximum, [4, 3, 4, 0]);
    assert!((uniforms.sampling[3] - 1.0).abs() < f32::EPSILON);
    assert_eq!(uniforms.clip_meta[0], 1);
}

#[test]
fn absent_direct_label_styles_are_zero_opacity_and_not_present() {
    let styles = SegmentStyleTable::new(&[SegmentStyle::new(2, Rgba8::WHITE, 0.75)])
        .expect("styles validate");
    let lookup = SegmentLookup::new(styles.styles());
    assert_eq!(lookup.mode(), LookupMode::Direct);
    for label in [0, 1] {
        assert_eq!(lookup.entries()[label].present, 0);
        assert!(lookup.entries()[label].opacity.abs() < f32::EPSILON);
        assert!(
            lookup.entries()[label]
                .color
                .iter()
                .all(|channel| channel.abs() < f32::EPSILON)
        );
    }
}

#[test]
fn absent_sparse_label_styles_and_empty_tables_are_transparent() {
    let styles = SegmentStyleTable::new(&[SegmentStyle::new(u32::MAX, Rgba8::WHITE, 0.75)])
        .expect("styles validate");
    let lookup = SegmentLookup::new(styles.styles());
    assert_eq!(lookup.mode(), LookupMode::Hash);
    for entry in lookup.entries().iter().filter(|entry| entry.present == 0) {
        assert!(entry.opacity.abs() < f32::EPSILON);
        assert!(
            entry
                .color
                .iter()
                .all(|channel| channel.abs() < f32::EPSILON)
        );
    }
    let empty = SegmentLookup::new(&[]);
    assert!(
        empty
            .entries()
            .iter()
            .all(|entry| entry.present == 0 && entry.opacity == 0.0)
    );
}
