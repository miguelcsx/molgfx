use super::*;

fn camera(eye: Vec3, target: Vec3) -> Camera {
    Camera {
        eye,
        target,
        up: Vec3::Y,
        projection: Projection::Perspective {
            fov_y: 0.8,
            aspect: 1.5,
            near: 0.1,
            far: 100.0,
        },
    }
}

#[test]
fn path_slerps_orientation_and_clamps_without_allocating_samples() {
    let start = camera(Vec3::new(0.0, 0.0, 10.0), Vec3::ZERO);
    let end = camera(Vec3::new(10.0, 0.0, 0.0), Vec3::ZERO);
    let path = CameraPath::new(
        Arc::from([
            CameraKeyframe::new(0.0, start).unwrap_or_else(|error| panic!("{error}")),
            CameraKeyframe::new(2.0, end).unwrap_or_else(|error| panic!("{error}")),
        ]),
        CameraEasing::Linear,
    )
    .unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(path.sample(-1.0), Some(start));
    assert_eq!(path.sample(3.0), Some(end));
    let middle = path.sample(1.0).unwrap_or_else(|| panic!("middle sample"));
    assert!((middle.focus_distance() - 10.0).abs() < 1.0e-5);
    assert!((middle.eye - Vec3::new(7.071_068, 0.0, 7.071_068)).length() < 1.0e-4);
}

#[test]
fn incompatible_projection_models_are_rejected() {
    let perspective = camera(Vec3::Z * 10.0, Vec3::ZERO);
    let mut orthographic = perspective;
    orthographic.projection = Projection::Orthographic {
        height: 10.0,
        aspect: 1.5,
        near: 0.1,
        far: 100.0,
    };
    let result = CameraPath::new(
        Arc::from([
            CameraKeyframe::new(0.0, perspective).unwrap_or_else(|error| panic!("{error}")),
            CameraKeyframe::new(1.0, orthographic).unwrap_or_else(|error| panic!("{error}")),
        ]),
        CameraEasing::SmoothStep,
    );
    assert!(matches!(result, Err(CoreError::InvalidTimeline { .. })));
}

#[test]
fn bookmark_json_round_trips_validated_camera_metadata() {
    let value = CameraBookmark::new("binding event", 1.25, camera(Vec3::Z * 8.0, Vec3::ZERO))
        .unwrap_or_else(|error| panic!("{error}"));
    let json = value.to_json().unwrap_or_else(|error| panic!("{error}"));
    let decoded = CameraBookmark::from_json(&json).unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(decoded, value);
}
