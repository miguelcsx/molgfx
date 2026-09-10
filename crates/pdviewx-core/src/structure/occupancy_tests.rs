use crate::{AtomSelection, CoreError, OccupancyStream, Representation, Scene};
use pdviewx_math::Vec3;

fn stream() -> OccupancyStream {
    OccupancyStream::new(
        [32, 24, 16],
        Vec3::new(-4.0, -3.0, -2.0),
        Vec3::splat(0.25),
        0.98,
        1.5,
        100.0,
    )
    .unwrap_or_else(|error| panic!("occupancy stream validates: {error}"))
}

#[test]
fn stream_is_a_compact_grid_declaration_without_host_voxels() {
    let value = stream();
    assert_eq!(value.dimensions(), [32, 24, 16]);
    assert_eq!(value.decay().to_bits(), 0.98f32.to_bits());
    assert_eq!(value.deposit().to_bits(), 1.5f32.to_bits());
    assert_eq!(value.maximum().to_bits(), 100.0f32.to_bits());
    assert_eq!(
        value.voxel_to_model().w_axis.truncate(),
        Vec3::new(-4.0, -3.0, -2.0)
    );
}

#[test]
fn malformed_stream_controls_are_typed_errors() {
    for (decay, deposit, maximum) in [
        (1.1, 1.0, 2.0),
        (0.9, 0.0, 2.0),
        (0.9, 3.0, 2.0),
        (0.9, 1.0, 1_000_001.0),
    ] {
        assert!(matches!(
            OccupancyStream::new([8; 3], Vec3::ZERO, Vec3::ONE, decay, deposit, maximum),
            Err(CoreError::InvalidVolume { .. })
        ));
    }
}

#[test]
fn scene_stores_only_normalized_rows_and_the_stream_declaration() {
    let structure = crate::fixture::structure();
    let mut scene = Scene::from_structure(&structure)
        .unwrap_or_else(|error| panic!("fixture scene builds: {error}"));
    let Some((structure_handle, _)) = scene.structures().next() else {
        panic!("fixture has one structure")
    };
    let volume = scene
        .add_occupancy_stream(
            structure_handle,
            &AtomSelection::Sparse(vec![2, 0, 2, u32::MAX]),
            stream(),
        )
        .unwrap_or_else(|error| panic!("occupancy binds: {error}"));
    let (_, owner, rows) = scene
        .occupancy_stream(volume)
        .unwrap_or_else(|| panic!("occupancy resolves"));
    assert_eq!(owner, structure_handle);
    assert_eq!(rows, &[0, 2]);
    assert!(scene.volume(volume).is_none());
    assert!(scene.represent(volume, Representation::volume()).is_ok());
    assert!(!scene.world_aabb().is_empty());
    scene
        .replace_occupancy_stream(
            volume,
            structure_handle,
            &AtomSelection::Sparse(vec![1]),
            OccupancyStream::new([8; 3], Vec3::splat(-2.0), Vec3::splat(0.5), 0.9, 2.0, 20.0)
                .unwrap_or_else(|error| panic!("replacement validates: {error}")),
        )
        .unwrap_or_else(|error| panic!("occupancy replaces: {error}"));
    let (replaced, _, rows) = scene
        .occupancy_stream(volume)
        .unwrap_or_else(|| panic!("replaced occupancy resolves"));
    assert_eq!(replaced.dimensions(), [8; 3]);
    assert_eq!(rows, &[1]);
}
