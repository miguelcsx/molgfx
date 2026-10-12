use crate::{Color, DataSource, IsoStyle, Scene, VolumeBinding, density};
use std::sync::Arc;

#[test]
fn orbital_lobes_share_one_amplitude_grid_and_keep_opposite_levels() {
    let mut scene = Scene::empty();
    let source = DataSource::new("sampled-wavefunction");
    scene
        .add(density::volume(source.clone(), [2; 3]).orbital(
            0.25,
            Color::rgb(255, 0, 0),
            Color::rgb(0, 0, 255),
            0.6,
            IsoStyle::Solid,
        ))
        .unwrap();
    scene
        .bind_volume(VolumeBinding::new(
            source,
            [2; 3],
            Arc::from([-1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0]),
        ))
        .unwrap();
    assert_eq!(scene.overlay_handles().volumes, 1);
    let representations: Vec<_> = scene
        .resolved()
        .representations()
        .map(|(_, representation)| representation)
        .collect();
    assert_eq!(representations.len(), 2);
    assert_eq!(representations[0].target, representations[1].target);
    assert_eq!(
        representations[0].params.isolevel.to_bits(),
        0.25_f32.to_bits()
    );
    assert_eq!(
        representations[1].params.isolevel.to_bits(),
        (-0.25_f32).to_bits()
    );
    assert_ne!(
        representations[0].volume.transfer,
        representations[1].volume.transfer
    );
    for representation in &representations {
        let points = representation.volume.transfer.points();
        assert!(points[0].value <= representation.params.isolevel);
        assert!(points[points.len() - 1].value >= representation.params.isolevel);
    }
}

#[test]
fn zero_negative_and_nonfinite_orbital_magnitudes_are_rejected_atomically() {
    for amplitude in [0.0, -0.25, f32::NAN, f32::INFINITY] {
        let mut scene = Scene::empty();
        let before = scene.to_spec();
        assert!(
            scene
                .add(density::volume(DataSource::new("orbital"), [2; 3]).orbital(
                    amplitude,
                    Color::rgb(255, 0, 0),
                    Color::rgb(0, 0, 255),
                    1.0,
                    IsoStyle::Solid,
                ))
                .is_err()
        );
        assert_eq!(scene.spec(), &before);
    }
}
