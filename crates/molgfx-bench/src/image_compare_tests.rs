use super::*;

const SIZE: u32 = 64;

fn checker(shift: u32) -> Vec<u8> {
    let mut out = Vec::new();
    for y in 0..SIZE {
        for x in 0..SIZE {
            let on = ((x + shift) / 4 + y / 4).is_multiple_of(2);
            let v = if on { 230 } else { 20 };
            out.extend_from_slice(&[v, v, v, 255]);
        }
    }
    out
}

fn passes(a: &[u8], b: &[u8]) -> bool {
    let de = delta_e76(a, b, SIZE, SIZE);
    ssim_luma(a, b, SIZE, SIZE) >= 0.985 && de.mean <= 1.0 && de.p99 <= 8.0
}

#[test]
fn identical_images_score_perfect_similarity_and_zero_difference() {
    let a = checker(0);
    assert!((ssim_luma(&a, &a, SIZE, SIZE) - 1.0).abs() < 1e-12);
    assert_eq!(
        delta_e76(&a, &a, SIZE, SIZE),
        DeltaEStats {
            mean: 0.0,
            p99: 0.0
        }
    );
}

#[test]
fn a_four_pixel_shift_of_a_checkerboard_fails_the_threshold() {
    assert!(!passes(&checker(0), &checker(4)));
}

#[test]
fn one_changed_pixel_keeps_similarity_and_leaves_the_percentile_alone() {
    let a = checker(0);
    let mut b = a.clone();
    b[..3].copy_from_slice(&[255, 0, 0]);
    assert!(ssim_luma(&a, &b, SIZE, SIZE) >= 0.985);
    assert!(delta_e76(&a, &b, SIZE, SIZE).p99 < 1e-9);
}

#[test]
fn changing_more_than_one_percent_of_pixels_moves_the_percentile() {
    let a = checker(0);
    let mut b = a.clone();
    for pixel in b.as_chunks_mut::<4>().0.iter_mut().take(205) {
        pixel[..3].copy_from_slice(&[255, 0, 0]);
    }
    assert!(delta_e76(&a, &b, SIZE, SIZE).p99 > 8.0);
}

#[test]
fn the_diff_image_is_a_grey_png_of_the_requested_extent() {
    let bytes = diff_png(&checker(0), &checker(4), SIZE, SIZE).expect("diff encodes");
    let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    let reader = decoder.read_info().expect("png decodes");
    let info = reader.info();
    assert_eq!(
        (info.width, info.height, info.color_type),
        (SIZE, SIZE, png::ColorType::Grayscale)
    );
}
