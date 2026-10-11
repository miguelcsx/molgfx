use super::*;
use std::sync::Arc;

fn linear_field(transform: Mat4) -> ScalarVolume {
    let mut values = Vec::new();
    for z in 0..5_u16 {
        for y in 0..5_u16 {
            for x in 0..5_u16 {
                values.push(
                    transform
                        .transform_point3(Vec3::new(f32::from(x), f32::from(y), f32::from(z)))
                        .x,
                );
            }
        }
    }
    ScalarVolume::new([5; 3], transform, Arc::from(values)).unwrap()
}

fn parameters() -> FieldLineParams {
    FieldLineParams {
        seed_stride: 2,
        step: 0.125,
        length: [0.25, 4.0],
        ..FieldLineParams::default()
    }
}

#[test]
fn a_linear_potential_follows_the_world_gradient_on_a_skew_grid() {
    let transform = Mat4::from_cols_array(&[
        2.0, 0.5, 0.0, 0.0, 1.0, 1.5, 0.0, 0.0, 0.0, 0.5, 3.0, 0.0, -1.0, 2.0, 4.0, 1.0,
    ]);
    let paths = field_lines(&linear_field(transform), parameters()).unwrap();
    assert!(!paths.is_empty());
    for path in paths {
        for pair in path.windows(2) {
            let direction = (pair[1] - pair[0]).normalize();
            assert!(direction.distance(Vec3::X) < 1.0e-4, "{direction:?}");
        }
    }
}

#[test]
fn a_constant_potential_has_no_field_lines() {
    let volume = ScalarVolume::new([5; 3], Mat4::IDENTITY, Arc::from([3.0; 125])).unwrap();
    assert!(field_lines(&volume, parameters()).unwrap().is_empty());
}

#[test]
fn a_cropped_field_stops_at_its_voxel_boundary() {
    let volume = linear_field(Mat4::IDENTITY);
    let region = VolumeRegion::new([1; 3], [4; 3], [5; 3]).unwrap();
    let paths = field_lines_in_region(&volume, parameters(), Some(region)).unwrap();
    assert!(!paths.is_empty());
    for point in paths.into_iter().flatten() {
        assert!(point.min_element() >= 1.0 && point.max_element() <= 3.0);
    }
}

#[test]
fn an_exhausted_segment_budget_is_reported_without_a_partial_result() {
    let params = FieldLineParams {
        max_segments: 1,
        ..parameters()
    };
    assert_eq!(
        field_lines(&linear_field(Mat4::IDENTITY), params),
        Err(FieldLineError::SegmentBudget)
    );
}

#[test]
fn invalid_steps_and_unbounded_seed_work_are_rejected() {
    let params = FieldLineParams {
        step: f32::NAN,
        ..parameters()
    };
    assert_eq!(params.validate(), Err(FieldLineError::InvalidParameters));
    let volume = ScalarVolume::new(
        [41; 3],
        Mat4::IDENTITY,
        Arc::from(vec![0.0; 41_usize.pow(3)]),
    )
    .unwrap();
    let params = FieldLineParams {
        seed_stride: 1,
        ..parameters()
    };
    assert_eq!(
        field_lines(&volume, params),
        Err(FieldLineError::SeedBudget)
    );
}
