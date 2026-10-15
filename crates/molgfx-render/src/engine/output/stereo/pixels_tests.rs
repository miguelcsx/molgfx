use super::*;

#[test]
fn stereo_layouts_preserve_eye_order_and_interleave_physical_pixels() {
    let left = [1, 2, 3, 255, 4, 5, 6, 255, 7, 8, 9, 255, 10, 11, 12, 255];
    let right = [
        21, 22, 23, 255, 24, 25, 26, 255, 27, 28, 29, 255, 30, 31, 32, 255,
    ];
    let (_, pixels) =
        compose(&left, &right, 2, 2, StereoLayout::SideBySide).expect("pair composes");
    assert_eq!(
        &pixels[..16],
        &[1, 2, 3, 255, 4, 5, 6, 255, 21, 22, 23, 255, 24, 25, 26, 255]
    );
    let (_, pixels) =
        compose(&left, &right, 2, 2, StereoLayout::CrossEyed).expect("cross pair composes");
    assert_eq!(&pixels[..8], &right[..8]);
    for (layout, expected) in [
        (StereoLayout::Checkerboard, [1, 24, 27, 10]),
        (StereoLayout::Columns, [1, 24, 7, 30]),
        (StereoLayout::Rows, [1, 4, 27, 30]),
    ] {
        let (_, pixels) = compose(&left, &right, 2, 2, layout).expect("interleaved pair composes");
        assert_eq!(
            pixels
                .as_chunks::<4>()
                .0
                .iter()
                .map(|p| p[0])
                .collect::<Vec<_>>(),
            expected
        );
    }
    let (_, pixels) =
        compose(&left, &right, 2, 2, StereoLayout::RedCyan).expect("anaglyph composes");
    assert_eq!(&pixels[..4], &[1, 22, 23, 255]);
    let (width, pixels) =
        compose(&left, &right, 2, 2, StereoLayout::HdSideBySide).expect("compressed pair composes");
    assert_eq!(width, 2);
    assert_eq!(&pixels[..8], &[2, 3, 4, 255, 22, 23, 24, 255]);
    assert!(compose(&left[..3], &right, 2, 2, StereoLayout::Left).is_err());
    assert!(compose(&left[..12], &right[..12], 3, 1, StereoLayout::HdSideBySide).is_err());
}
