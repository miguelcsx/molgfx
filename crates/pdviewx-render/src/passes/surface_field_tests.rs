use super::*;

#[test]
fn surface_field_dispatch_covers_each_voxel_axis_once() {
    assert_eq!(dispatch_grid([1, 1, 1]), [1, 1, 1]);
    assert_eq!(dispatch_grid([4, 5, 192]), [1, 2, 48]);
    assert_eq!(dispatch_grid([191, 192, 193]), [48, 48, 49]);
}
