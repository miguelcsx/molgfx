use super::*;
use crate::ClipPlane;
use pdviewx_math::Vec3;

#[test]
fn volume_transfer_points_remain_ordered_and_bounded() {
    let points = [
        VolumeTransferPoint::new(-1.0, Rgba8::opaque(0, 0, 255), 0.0),
        VolumeTransferPoint::new(0.0, Rgba8::opaque(255, 255, 255), 0.2),
        VolumeTransferPoint::new(1.0, Rgba8::opaque(255, 0, 0), 0.9),
    ];
    let transfer = match VolumeTransferFunction::new(&points) {
        Ok(transfer) => transfer,
        Err(error) => panic!("valid transfer builds: {error}"),
    };
    assert_eq!(transfer.points(), points.as_slice());
}

#[test]
fn malformed_volume_transfer_functions_are_typed_errors() {
    let point = VolumeTransferPoint::new(0.0, Rgba8::WHITE, 0.5);
    let Err(short) = VolumeTransferFunction::new(&[point]) else {
        panic!("one control point is ambiguous")
    };
    assert_eq!(short.code(), "PDVIEWX-E0032");
    let Err(unordered) = VolumeTransferFunction::new(&[point, point]) else {
        panic!("duplicate scalar values are ambiguous")
    };
    assert_eq!(unordered.code(), "PDVIEWX-E0032");
    let Err(opacity) =
        VolumeTransferFunction::new(&[point, VolumeTransferPoint::new(1.0, Rgba8::WHITE, 1.1)])
    else {
        panic!("opacity outside the unit interval is invalid")
    };
    assert_eq!(opacity.code(), "PDVIEWX-E0032");
}

#[test]
fn a_volume_slice_keeps_its_validated_world_plane() {
    let plane = match ClipPlane::from_point_normal(Vec3::new(2.0, 0.0, 0.0), Vec3::X) {
        Ok(plane) => plane,
        Err(error) => panic!("slice plane builds: {error}"),
    };
    let slice = VolumeSlice::new(plane);
    assert!(slice.plane.signed_distance(Vec3::new(2.0, 8.0, -4.0)).abs() < f32::EPSILON);
}

#[test]
fn a_volume_region_is_half_open_and_cannot_escape_its_grid() {
    let region = match VolumeRegion::new([2, 3, 4], [8, 9, 10], [12, 12, 12]) {
        Ok(region) => region,
        Err(error) => panic!("region builds: {error}"),
    };
    assert_eq!(region.minimum(), [2, 3, 4]);
    assert_eq!(region.maximum(), [8, 9, 10]);
    assert!(VolumeRegion::new([2, 3, 4], [2, 9, 10], [12, 12, 12]).is_err());
    assert!(VolumeRegion::new([2, 3, 4], [13, 9, 10], [12, 12, 12]).is_err());
}

#[test]
fn gaussian_surface_parameters_keep_width_and_iso_level_independent() {
    let params = RepresentationParams::default();
    assert_eq!(params.gaussian_sigma.to_bits(), 1.0f32.to_bits());
    assert_eq!(params.surface_kind, SurfaceKind::SolventExcluded);
    assert_eq!(SurfaceKind::Gaussian as u32, 3);
}
