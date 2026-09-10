use super::workgroups_2d;

#[test]
fn portable_dispatch_spills_large_linear_workloads_into_y() {
    assert_eq!(workgroups_2d(0), [0, 0]);
    assert_eq!(workgroups_2d(65_535), [65_535, 1]);
    assert_eq!(workgroups_2d(65_536), [32_768, 2]);
    assert_eq!(workgroups_2d(1_562_500), [65_105, 24]);
}

#[test]
fn portable_dispatch_covers_the_largest_coordinate_table() {
    let total = u64::from(u32::MAX).div_ceil(64);
    let groups = workgroups_2d(total);

    assert!(groups.into_iter().all(|axis| axis <= 65_535));
    assert!(u64::from(groups[0]) * u64::from(groups[1]) >= total);
}
