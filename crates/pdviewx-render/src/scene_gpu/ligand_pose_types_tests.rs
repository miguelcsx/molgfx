use super::*;
use pdviewx_math::{Mat4, Quat, Rgba8, Vec3};

fn pose(alpha: u8, opacity: f32) -> LigandPose {
    let result = LigandPose::new(
        Vec3::new(1.0, 2.0, 3.0),
        Quat::IDENTITY,
        Rgba8::new(10, 20, 30, alpha),
        opacity,
    );
    let Ok(pose) = result else {
        panic!("valid pose expected");
    };
    pose
}

#[test]
fn pose_columns_keep_exact_transform_color_and_opacity() {
    let pose = pose(128, 0.375);
    let transform = PoseTransformGpu::from(pose);
    let style = PoseStyleGpu::from(pose);

    assert_eq!(
        transform.translation_opacity.map(f32::to_bits),
        [1.0, 2.0, 3.0, 0.375].map(f32::to_bits),
    );
    assert_eq!(
        transform.orientation.map(f32::to_bits),
        Quat::IDENTITY.to_array().map(f32::to_bits),
    );
    assert_eq!(style.0, 0x801e_140a);
    assert_eq!(
        std::mem::size_of::<PoseTransformGpu>() + std::mem::size_of::<PoseStyleGpu>(),
        36,
    );
}

#[test]
fn pose_classification_matches_final_alpha() {
    let mut opaque_indices = Vec::new();
    let mut translucent_indices = Vec::new();
    let ranges = classify_poses(
        &[
            pose(255, 1.0),
            pose(255, 0.9995),
            pose(128, 1.0),
            pose(255, 0.5),
            pose(0, 1.0),
            pose(255, 0.0),
        ],
        POSE_HASH_OFFSET,
        &mut opaque_indices,
        &mut translucent_indices,
    );
    let Ok(ranges) = ranges else {
        panic!("small pose range expected");
    };

    assert_eq!(ranges.opaque, 1);
    assert_eq!(ranges.translucent, 3);
    assert_eq!(opaque_indices, vec![0]);
    assert_eq!(translucent_indices, vec![1, 2, 3]);
    let Ok(visible) = ranges.visible() else {
        panic!("visible pose count expected");
    };
    assert_eq!(visible, 4);
    assert!(pose_is_translucent(pose(255, 0.9995)));
    assert!(!pose_is_visible(pose(0, 1.0)));
    assert!(!pose_is_visible(pose(255, 0.0)));
}

#[test]
fn batch_record_keeps_source_and_sampled_opacity_ranges() {
    let batch = PoseBatchGpu::new(
        [1, 2, 3, 4],
        [5, 11],
        [7, 3, 4, 2],
        batch_entity(9).unwrap_or_else(|error| panic!("batch entity fits: {error}")),
        13,
        [0.5, 0.25],
        Mat4::IDENTITY,
    );

    assert_eq!(batch.sampling, [7, 3, 4, 2]);
    assert_eq!(batch.poses[..2], [5, 11]);
    assert_eq!(std::mem::size_of::<PoseBatchGpu>(), 128);
}

#[test]
fn indirect_ranges_encode_batch_and_sampled_topology_without_saturation() {
    let mut args = Vec::new();
    let mut groups = Vec::new();
    let result = append_draw(
        &mut args,
        &mut groups,
        PoseDrawRange {
            batch_index: 7,
            shape: POSE_CAPSULE,
            topology_count: 11,
            pose_count: 17,
            translucent: true,
        },
    );
    assert!(result.is_ok());
    assert_eq!(args[0].first_vertex, 42);
    assert_eq!(args[0].first_instance, 0);
    assert_eq!(args[0].instance_count, 187);
    assert_eq!(groups[0].args_offset, 0);
}

#[test]
fn indirect_instance_overflow_is_an_explicit_limit_error() {
    let mut args = Vec::new();
    let mut groups = Vec::new();
    let result = append_draw(
        &mut args,
        &mut groups,
        PoseDrawRange {
            batch_index: 0,
            shape: POSE_SPHERE,
            topology_count: u32::MAX,
            pose_count: 2,
            translucent: false,
        },
    );

    assert!(matches!(
        result,
        Err(RenderError::Gpu(pdviewx_gpu::GpuError::LimitExceeded {
            resource: "ligand pose draw instances",
            ..
        }))
    ));
    assert!(args.is_empty());
    assert!(groups.is_empty());
}

#[test]
fn batch_vertex_overflow_is_an_explicit_limit_error() {
    let mut args = Vec::new();
    let mut groups = Vec::new();
    let result = append_draw(
        &mut args,
        &mut groups,
        PoseDrawRange {
            batch_index: u32::MAX,
            shape: POSE_SPHERE,
            topology_count: 1,
            pose_count: 1,
            translucent: false,
        },
    );

    assert!(matches!(
        result,
        Err(RenderError::Gpu(pdviewx_gpu::GpuError::LimitExceeded {
            resource: "ligand pose batch index",
            ..
        }))
    ));
}
