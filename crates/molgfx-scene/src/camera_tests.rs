use super::{CameraEasing, path, perspective};

fn camera(x: f32) -> crate::Camera {
    match perspective(
        [x, 0.0, 10.0],
        [0.0; 3],
        [0.0, 1.0, 0.0],
        0.8,
        1.5,
        0.1,
        100.0,
    ) {
        Ok(camera) => camera,
        Err(error) => panic!("the camera is valid: {error}"),
    }
}

#[test]
fn a_path_samples_between_keyframes_and_clamps_at_its_ends() {
    let Ok(path) = path(
        &[(0.0, camera(0.0)), (2.0, camera(8.0))],
        CameraEasing::Linear,
    ) else {
        panic!("two ordered keyframes make a path")
    };
    let [start, end] = path.range();
    assert!(start.abs() < 1e-9 && (end - 2.0).abs() < 1e-9);
    let middle = path.sample(1.0).map(|camera| camera.eye.x);
    // Paths orbit the target, so the midpoint lies strictly between the ends
    // without being their arithmetic mean.
    assert!(middle.is_some_and(|x| x > 1.0 && x < 7.0), "{middle:?}");
    let before = path.sample(-5.0).map(|camera| camera.eye.x);
    let after = path.sample(9.0).map(|camera| camera.eye.x);
    assert!(before.is_some_and(|x| x.abs() < 1e-6), "{before:?}");
    assert!(after.is_some_and(|x| (x - 8.0).abs() < 1e-6), "{after:?}");
    assert!(path.sample(f64::NAN).is_none());
}

#[test]
fn malformed_paths_are_rejected_before_any_sampling() {
    let one = [(0.0, camera(0.0))];
    assert!(path(&one, CameraEasing::Linear).is_err());
    let unordered = [(1.0, camera(0.0)), (1.0, camera(1.0))];
    assert!(path(&unordered, CameraEasing::Linear).is_err());
    let backwards = [(2.0, camera(0.0)), (1.0, camera(1.0))];
    assert!(path(&backwards, CameraEasing::SmoothStep).is_err());
}
