use super::{RAMP_LUT_SIZE, RampLut};
use molgfx_core::ScalarRamp;
use molgfx_math::Rgba8;

fn unpack(word: u32) -> Rgba8 {
    let [r, g, b, a] = word.to_le_bytes();
    Rgba8::new(r, g, b, a)
}

fn ramp() -> ScalarRamp {
    let colors = [
        Rgba8::opaque(0, 0, 255),
        Rgba8::opaque(0, 255, 0),
        Rgba8::opaque(255, 255, 0),
        Rgba8::opaque(255, 0, 0),
    ];
    match ScalarRamp::new(&[0.0, 1.0, 3.0, 4.0], &colors) {
        Ok(ramp) => ramp,
        Err(error) => panic!("ramp builds: {error}"),
    }
}

#[test]
fn the_table_reproduces_the_ramp_at_its_ends_and_between_stops() {
    let ramp = ramp();
    let missing = Rgba8::opaque(9, 9, 9);
    let lut = RampLut::new(&ramp, missing);
    assert_eq!(unpack(lut.entry(0)), Rgba8::opaque(0, 0, 255));
    assert_eq!(
        unpack(lut.entry(RAMP_LUT_SIZE - 1)),
        Rgba8::opaque(255, 0, 0)
    );
    let [first, scale] = lut.domain_probe();
    for index in [17_usize, 64, 100, 190, 240] {
        let value = first + u16::try_from(index).map_or(0.0, f32::from) / scale;
        let expected = ramp.sample(value, missing);
        let actual = unpack(lut.entry(index));
        for (a, e) in [
            (actual.r, expected.r),
            (actual.g, expected.g),
            (actual.b, expected.b),
        ] {
            assert!(
                a.abs_diff(e) <= 1,
                "entry {index}: {actual:?} vs {expected:?}"
            );
        }
    }
}

#[test]
fn a_disabled_table_is_all_zero() {
    let lut = RampLut::disabled();
    assert_eq!(lut.entry(0), 0);
    assert_eq!(lut.entry(RAMP_LUT_SIZE - 1), 0);
}
