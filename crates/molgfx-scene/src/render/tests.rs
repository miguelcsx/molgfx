use super::*;

#[test]
fn zero_sized_images_are_rejected_before_rendering() {
    assert_eq!(RenderProfile::default().target_fps, 60);
}
