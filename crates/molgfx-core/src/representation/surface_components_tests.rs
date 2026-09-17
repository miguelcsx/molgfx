use super::*;

#[test]
fn policies_reject_non_positive_and_non_finite_thresholds() {
    assert!(matches!(
        SurfaceComponentPolicy::minimum_area(f64::NAN),
        Err(SurfaceComponentPolicyError::NonFinite { measure: "area" })
    ));
    assert!(matches!(
        SurfaceComponentPolicy::minimum_volume(0.0),
        Err(SurfaceComponentPolicyError::NonPositive { measure: "volume" })
    ));
    assert_eq!(
        SurfaceComponentPolicy::minimum_voxels(0),
        Err(SurfaceComponentPolicyError::ZeroVoxels)
    );
}

#[test]
fn each_measure_is_preserved_without_loss() {
    let Ok(area) = SurfaceComponentPolicy::minimum_area(2.5) else {
        panic!("positive area validates")
    };
    let Ok(volume) = SurfaceComponentPolicy::minimum_volume(3.75) else {
        panic!("positive volume validates")
    };
    let Ok(voxels) = SurfaceComponentPolicy::minimum_voxels(u64::from(u32::MAX) + 17) else {
        panic!("u64 voxel threshold validates")
    };
    assert_eq!(area.threshold(), SurfaceComponentThreshold::Area(2.5));
    assert_eq!(volume.threshold(), SurfaceComponentThreshold::Volume(3.75));
    assert_eq!(
        voxels.threshold(),
        SurfaceComponentThreshold::Voxels(u64::from(u32::MAX) + 17)
    );
}
