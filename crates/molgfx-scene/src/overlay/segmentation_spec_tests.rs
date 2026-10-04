use super::{SegmentStyle, SegmentationSpec};
use crate::{Color, DataSource, SegmentationBinding};
use molgfx_math::Mat4;
use std::sync::Arc;

fn descriptor() -> SegmentationSpec {
    SegmentationSpec {
        presentation: crate::SegmentationPresentation::Surface,
        source: DataSource::new("categorical"),
        dimensions: [2; 3],
        voxel_to_world: Mat4::IDENTITY.to_cols_array(),
        styles: vec![SegmentStyle {
            label: 42,
            color: Color::rgb(230, 30, 50),
            opacity: 0.7,
            visible: true,
        }],
    }
}

#[test]
fn categorical_styles_reject_duplicate_labels_and_invalid_hidden_opacity() {
    let mut spec = descriptor();
    spec.styles.push(spec.styles[0]);
    assert!(spec.validate().is_err());
    spec.styles.pop();
    spec.styles[0].visible = false;
    spec.styles[0].opacity = f32::NAN;
    assert!(spec.validate().is_err());
}

#[test]
fn a_categorical_binding_rejects_mismatched_affine_and_label_count() {
    let spec = descriptor();
    assert!(
        SegmentationBinding::new(
            spec.source.clone(),
            spec.dimensions,
            spec.voxel_to_world,
            Arc::from([42; 7])
        )
        .is_err()
    );
    let binding = SegmentationBinding::new(
        spec.source.clone(),
        spec.dimensions,
        spec.voxel_to_world,
        Arc::from([42; 8]),
    )
    .unwrap();
    let mut wrong = spec;
    wrong.voxel_to_world[12] = 1.0;
    assert!(binding.matches(&wrong).is_err());
}

#[test]
fn empty_and_unlisted_styles_are_transparent() {
    let table = super::native_styles(&[]).unwrap();
    assert!(table.styles().is_empty());
    assert_eq!(table.style_for(42), None);
    let mut spec = descriptor();
    spec.styles[0].visible = false;
    let table = super::native_styles(&spec.styles).unwrap();
    assert!(table.style_for(42).unwrap().opacity.abs() < f32::EPSILON);
}
