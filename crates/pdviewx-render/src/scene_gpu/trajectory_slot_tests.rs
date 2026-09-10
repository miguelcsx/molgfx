use super::trajectory_dispatch_groups;

#[test]
fn trajectory_dispatch_covers_all_three_coordinate_lanes_per_atom() {
    assert_eq!(trajectory_dispatch_groups(1), [1, 1]);
    assert_eq!(trajectory_dispatch_groups(21), [1, 1]);
    assert_eq!(trajectory_dispatch_groups(22), [2, 1]);
    assert_eq!(trajectory_dispatch_groups(64), [3, 1]);
    assert_eq!(trajectory_dispatch_groups(1_398_080), [65_535, 1]);
    assert_eq!(trajectory_dispatch_groups(1_398_081), [32_768, 2]);
    assert_eq!(trajectory_dispatch_groups(2_440_800), [57_207, 2]);
}
