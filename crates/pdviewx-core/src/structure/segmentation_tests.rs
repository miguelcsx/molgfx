use super::{SegmentStyle, SegmentStyleTable, SegmentedVolume};
use crate::CoreError;
use pdviewx_math::{Mat4, Rgba8, Vec3};
use std::sync::Arc;

#[test]
fn categorical_volume_retains_shared_labels_and_world_bounds() {
    let labels: Arc<[u32]> = Arc::from([0, 1, 1, 2, 2, 0, 3, 3]);
    let pointer = labels.as_ptr();
    let volume = match SegmentedVolume::from_spacing(
        [2, 2, 2],
        Vec3::new(-1.0, 2.0, 3.0),
        Vec3::new(2.0, 3.0, 4.0),
        Arc::clone(&labels),
    ) {
        Ok(volume) => volume,
        Err(error) => panic!("categorical volume builds: {error}"),
    };
    assert_eq!(volume.labels().as_ptr(), pointer);
    assert_eq!(volume.dimensions(), [2, 2, 2]);
    assert_eq!(volume.world_aabb().min, Vec3::new(-1.0, 2.0, 3.0));
    assert_eq!(volume.world_aabb().max, Vec3::new(1.0, 5.0, 7.0));
}

#[test]
fn categorical_volume_rejects_malformed_grid_contracts() {
    let wrong_count = SegmentedVolume::new([2, 2, 2], Mat4::IDENTITY, Arc::from([0; 7]));
    assert!(matches!(
        wrong_count,
        Err(CoreError::InvalidSegmentation { .. })
    ));
    let singular = SegmentedVolume::new([2, 2, 2], Mat4::ZERO, Arc::from([0; 8]));
    assert!(matches!(
        singular,
        Err(CoreError::InvalidSegmentation { .. })
    ));
}

#[test]
fn segment_style_table_sorts_and_resolves_exact_labels() {
    let table = match SegmentStyleTable::new(&[
        SegmentStyle::new(9, Rgba8::opaque(9, 9, 9), 0.5),
        SegmentStyle::new(2, Rgba8::opaque(2, 2, 2), 1.0),
    ]) {
        Ok(table) => table,
        Err(error) => panic!("style table builds: {error}"),
    };
    assert_eq!(
        table
            .styles()
            .iter()
            .map(|style| style.label)
            .collect::<Vec<_>>(),
        [2, 9]
    );
    assert_eq!(table.style_for(2).map(|style| style.opacity), Some(1.0));
    assert!(table.style_for(3).is_none());
}

#[test]
fn segment_style_table_rejects_duplicate_and_invalid_opacity() {
    let duplicate = SegmentStyleTable::new(&[
        SegmentStyle::new(4, Rgba8::WHITE, 1.0),
        SegmentStyle::new(4, Rgba8::WHITE, 0.0),
    ]);
    assert!(matches!(
        duplicate,
        Err(CoreError::InvalidSegmentation { .. })
    ));
    let invalid = SegmentStyleTable::new(&[SegmentStyle::new(4, Rgba8::WHITE, f32::NAN)]);
    assert!(matches!(
        invalid,
        Err(CoreError::InvalidSegmentation { .. })
    ));
}
