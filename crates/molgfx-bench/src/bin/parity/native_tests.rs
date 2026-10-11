//! Observable native screenshot validity.
use super::validate_pixels;

#[test]
fn opaque_and_gradient_backgrounds_fail_without_visible_geometry() {
    assert!(validate_pixels(&[], &[]).is_err());
    let background = [10, 20, 30, 255, 20, 30, 40, 255];
    assert!(validate_pixels(&background, &background).is_err());
    assert!(validate_pixels(&[11, 20, 30, 255, 20, 29, 40, 255], &background).is_err());
    assert!(validate_pixels(&[0; 4], &background).is_err());
    assert!(validate_pixels(&[40, 20, 30, 255, 20, 30, 40, 255], &background).is_ok());
}
