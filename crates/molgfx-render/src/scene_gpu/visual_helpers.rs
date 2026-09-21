//! Small pure helpers for visual-program buffer layouts.

use molgfx_core::Representation;

const RESULT_OFFSET: u32 = 1 << 4;

pub(super) fn result_word_count(layout: [u32; 4]) -> u32 {
    let packed_words = (layout[0] & 0x0f).count_ones().saturating_mul(layout[3]);
    packed_words.saturating_add(
        u32::from(layout[0] & RESULT_OFFSET != 0).saturating_mul(layout[3].saturating_mul(3)),
    )
}

pub(super) fn saturating_u32(value: usize) -> u32 {
    crate::fallback(u32::try_from(value), u32::MAX)
}

pub(super) fn representation_base_color(representation: &Representation) -> [f32; 4] {
    match representation.color {
        molgfx_core::ColorScheme::Uniform(color) => {
            let scale = 1.0 / 255.0;
            [
                f32::from(color.r) * scale,
                f32::from(color.g) * scale,
                f32::from(color.b) * scale,
                f32::from(color.a) * scale,
            ]
        }
        _ => [1.0; 4],
    }
}
