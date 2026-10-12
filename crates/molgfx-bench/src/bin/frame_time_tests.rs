use super::*;

#[test]
fn zoom_divides_the_fitted_eye_distance_and_preserves_the_target() {
    let scene = Scene::empty();
    let mut options = Options {
        source: Source::File {
            path: String::new(),
            form: String::from("sticks"),
        },
        output: MeasuredOutput::Interactive,
        orbit_degrees: 0.5,
        warmup: 1,
        frames: 1,
        size: (1280, 720),
        zoom: 1.0,
    };
    let fitted = scene.framing_camera(aspect(options.size).unwrap());
    assert_eq!(options.camera(&scene).unwrap(), fitted);
    options.zoom = 6.0;
    let close = options.camera(&scene).unwrap();
    assert_eq!(close.target, fitted.target);
    assert!((close.focus_distance() * options.zoom - fitted.focus_distance()).abs() < 0.0001);
}
