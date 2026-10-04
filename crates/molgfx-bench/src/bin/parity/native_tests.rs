//! Observable native screenshot validity.
use super::validate_pixels;

#[test]
fn empty_or_all_zero_gallery_captures_fail_even_when_quality_metadata_is_complete() {
    assert!(validate_pixels(&[]).is_err());
    assert!(validate_pixels(&[0; 16]).is_err());
    assert!(validate_pixels(&[0, 0, 0, 255]).is_ok());
    assert!(validate_pixels(&[1, 2, 3, 0]).is_ok());
}
