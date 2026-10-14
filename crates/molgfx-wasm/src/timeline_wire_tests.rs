use crate::timeline_wire::{
    Easing, Mode, READING_STRIDE, WireError, pack_rows, parse_easing, parse_mode, parse_range,
};

#[test]
fn modes_and_easings_parse_from_their_fixed_names() {
    assert_eq!(parse_mode("pingpong"), Ok(Mode::PingPong));
    assert_eq!(parse_easing("smoothstep"), Ok(Easing::SmoothStep));
    assert!(matches!(parse_mode("bounce"), Err(WireError::Mode(_))));
    assert!(matches!(parse_easing("cubic"), Err(WireError::Easing(_))));
}

#[test]
fn rows_pack_into_a_reused_buffer_without_touching_the_tail() {
    let rows = [[7.0, 1.0e-9, 0.25, 3.0], [9.0, 2.0e-15, 1.0, 1.0]];
    let mut out = [f64::NAN; 2 * READING_STRIDE + 1];
    assert_eq!(pack_rows(|| rows.iter().copied(), &mut out), Ok(2));
    assert_eq!(out.first_chunk::<READING_STRIDE>(), Some(&rows[0]));
    assert_eq!(
        out.get(READING_STRIDE..2 * READING_STRIDE),
        Some(&rows[1][..])
    );
    assert!(out.last().is_some_and(|value| value.is_nan()));
}

#[test]
fn a_short_buffer_is_refused_and_left_untouched() {
    let rows = [[1.0, 1.0, 0.5, 6.0]];
    let mut out = [f64::NAN; READING_STRIDE - 1];
    assert_eq!(
        pack_rows(|| rows.iter().copied(), &mut out),
        Err(WireError::Buffer {
            needed: READING_STRIDE,
            offered: READING_STRIDE - 1
        })
    );
    assert!(out.iter().all(|value| value.is_nan()));
}

#[test]
fn a_time_range_needs_exactly_start_and_end() {
    assert_eq!(parse_range(&[1.0, 2.0]), Ok([1.0, 2.0]));
    assert_eq!(parse_range(&[1.0]), Err(WireError::Range(1)));
}
