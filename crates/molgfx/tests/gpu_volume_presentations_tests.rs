//! Native rendering checks for scalar-field presentations through the public API.

use molgfx::schema::DataSource;
use molgfx::{
    Color, FieldLines, IsoStyle, Renderer, Scene, VolumeBinding, camera, density, profile,
};
use std::sync::Arc;

fn renderer() -> Renderer {
    let profile = profile::converged()
        .with_effect(profile::Effect::AntiAliasing(profile::AntiAliasing::Off))
        .unwrap()
        .with_effect(profile::Effect::Backdrop(
            profile::BackdropStyle::transparent(),
        ))
        .unwrap();
    Renderer::with_profile(profile).unwrap()
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn scalar_gradient_streamlines_follow_replaced_potentials_without_atoms() {
    let mut scene = Scene::empty();
    let source = DataSource::new("linear-potential");
    let options = FieldLines {
        seed_stride: 5,
        step: 0.125,
        length: [0.25, 4.0],
        width_pixels: 3.0,
        color: Color::rgb(0, 120, 255),
        ..FieldLines::default()
    };
    let id = scene
        .add(density::volume(source.clone(), [5; 3]).field_lines(options))
        .unwrap();
    let values: Arc<[f32]> = (0..125_u16).map(|row| f32::from(row % 5)).collect();
    scene
        .bind_volume(VolumeBinding::new(source.clone(), [5; 3], values))
        .unwrap();
    let camera = camera::perspective(
        [2.0, 2.0, 16.0],
        [2.0; 3],
        [0.0, 1.0, 0.0],
        0.6,
        1.0,
        0.1,
        40.0,
    )
    .unwrap();
    let mut renderer = renderer();
    let image = renderer
        .render_image_with_camera(&scene, &camera, (128, 128))
        .unwrap();
    assert!(
        image
            .pixels()
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|pixel| pixel[3] > 0)
            .count()
            > 100
    );
    let horizontal = extent(&image);
    assert!(
        horizontal[0] > horizontal[1] * 3,
        "x potential must integrate along x"
    );
    scene
        .apply(&molgfx::ScenePatch {
            base_revision: scene.revision(),
            operations: vec![molgfx::schema::PatchOperation::RemoveVolume { id }],
        })
        .unwrap();
    let source = DataSource::new("y-potential");
    scene
        .add(density::volume(source.clone(), [5; 3]).field_lines(options))
        .unwrap();
    let values: Arc<[f32]> = (0..125_u16).map(|row| f32::from(row / 5 % 5)).collect();
    scene
        .bind_volume(VolumeBinding::new(source, [5; 3], values))
        .unwrap();
    let image = renderer
        .render_image_with_camera(&scene, &camera, (128, 128))
        .unwrap();
    let vertical = extent(&image);
    assert!(
        vertical[1] > vertical[0] * 3,
        "y potential must integrate along y"
    );
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn opposite_wavefunction_phases_render_as_separate_colored_lobes() {
    let mut scene = Scene::empty();
    let source = DataSource::new("sampled-wavefunction");
    scene
        .add(density::volume(source.clone(), [9; 3]).orbital(
            0.2,
            Color::rgb(255, 0, 0),
            Color::rgb(0, 0, 255),
            1.0,
            IsoStyle::Solid,
        ))
        .unwrap();
    let values: Arc<[f32]> = (0..729_u16)
        .map(|row| {
            let x = f32::from(row % 9);
            let y = f32::from(row / 9 % 9) - 4.0;
            let z = f32::from(row / 81) - 4.0;
            (-((x - 2.0).powi(2) + y * y + z * z) / 2.0).exp()
                - (-((x - 6.0).powi(2) + y * y + z * z) / 2.0).exp()
        })
        .collect();
    scene
        .bind_volume(VolumeBinding::new(source, [9; 3], values))
        .unwrap();
    let camera = camera::perspective(
        [4.0, 4.0, 24.0],
        [4.0; 3],
        [0.0, 1.0, 0.0],
        0.6,
        1.0,
        0.1,
        40.0,
    )
    .unwrap();
    let mut renderer = renderer();
    let image = renderer
        .render_image_with_camera(&scene, &camera, (128, 128))
        .unwrap();
    let pixels: Vec<_> = image
        .pixels()
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|pixel| pixel[3] > 0)
        .collect();
    assert!(
        pixels
            .iter()
            .filter(|pixel| i16::from(pixel[0]) - i16::from(pixel[2]) > 20)
            .count()
            > 100
    );
    assert!(
        pixels
            .iter()
            .filter(|pixel| i16::from(pixel[2]) - i16::from(pixel[0]) > 20)
            .count()
            > 100
    );
    let repeat = renderer
        .render_image_with_camera(&scene, &camera, (128, 128))
        .unwrap();
    assert!(
        image.pixels() == repeat.pixels(),
        "unchanged field differed in {} channels, max {}",
        image
            .pixels()
            .iter()
            .zip(repeat.pixels())
            .filter(|(a, b)| a != b)
            .count(),
        image
            .pixels()
            .iter()
            .zip(repeat.pixels())
            .map(|(a, b)| a.abs_diff(*b))
            .max()
            .unwrap_or(0)
    );
}

#[test]
#[ignore = "requires a native GPU adapter"]
fn overlapping_field_lines_repeat_exactly_on_the_same_adapter() {
    let mut scene = Scene::empty();
    let source = DataSource::new("dense-linear-potential");
    scene
        .add(
            density::volume(source.clone(), [5; 3]).field_lines(FieldLines {
                seed_stride: 1,
                step: 0.125,
                length: [0.25, 4.0],
                width_pixels: 3.0,
                color: Color::rgb(0, 120, 255),
                ..FieldLines::default()
            }),
        )
        .expect("field descriptor");
    let values: Arc<[f32]> = (0..125_u16).map(|row| f32::from(row % 5)).collect();
    scene
        .bind_volume(VolumeBinding::new(source, [5; 3], values))
        .expect("field data");
    let camera = camera::perspective(
        [2.0, 2.0, 16.0],
        [2.0; 3],
        [0.0, 1.0, 0.0],
        0.6,
        1.0,
        0.1,
        40.0,
    )
    .expect("camera");
    let mut renderer = renderer();
    let first = renderer
        .render_image_with_camera(&scene, &camera, (128, 128))
        .expect("reference frame");
    for _ in 0..4 {
        let repeat = renderer
            .render_image_with_camera(&scene, &camera, (128, 128))
            .expect("repeat frame");
        assert_eq!(
            first
                .pixels()
                .iter()
                .zip(repeat.pixels())
                .filter(|(a, b)| a != b)
                .count(),
            0,
            "unchanged overlapping strokes must not depend on GPU compaction order"
        );
    }
}

fn extent(image: &molgfx::Image) -> [usize; 2] {
    let width = usize::try_from(image.width()).unwrap();
    let mut minimum = [usize::MAX; 2];
    let mut maximum = [0; 2];
    for (row, pixel) in image.pixels().as_chunks::<4>().0.iter().enumerate() {
        if pixel[3] < 128 {
            continue;
        }
        let point = [row % width, row / width];
        for axis in 0..2 {
            minimum[axis] = minimum[axis].min(point[axis]);
            maximum[axis] = maximum[axis].max(point[axis]);
        }
    }
    assert!(minimum[0] != usize::MAX, "presentation must cover pixels");
    [maximum[0] - minimum[0] + 1, maximum[1] - minimum[1] + 1]
}
