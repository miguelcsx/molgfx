//! Binary records shared by label packing, decluttering and rendering.

use pdviewx_math::{Rgba8, Vec3};

pub(super) const OCCUPANCY_SLOTS: u64 = 4096;
pub(super) const GLYPH_WIDTH: f32 = 10.0;
pub(super) const GLYPH_HEIGHT: f32 = 16.0;
pub(super) const GLYPH_ADVANCE: f32 = 11.0;

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct LabelHeaderGpu {
    pub anchor_width: [f32; 4],
    pub bounds: [f32; 4],
    pub range: [u32; 4],
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct LabelGpu {
    pub start_size: [f32; 4],
    pub end_offset: [f32; 4],
    pub color: [f32; 4],
    pub metadata: [u32; 4],
}

#[repr(C, align(16))]
#[derive(Clone, Copy, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct LabelCountsGpu {
    pub header_count: u32,
    pub source_count: u32,
    pub occupancy_count: u32,
    pub padding: u32,
}

pub(super) fn glyph_record(
    anchor: Vec3,
    offset: [f32; 2],
    bits: u64,
    color: Rgba8,
    entity: u32,
    structure: u32,
) -> LabelGpu {
    let low = u32::try_from(bits & u64::from(u32::MAX)).map_or(u32::MAX, |value| value);
    let high = u32::try_from(bits >> 32).map_or(u32::MAX, |value| value);
    LabelGpu {
        start_size: anchor.extend(GLYPH_WIDTH).to_array(),
        end_offset: [offset[0], offset[1], f32::from_bits(high), GLYPH_HEIGHT],
        color: color.to_f32(),
        metadata: [entity, structure, 0, low],
    }
}

pub(super) fn line_record(
    start: Vec3,
    end: Vec3,
    width: f32,
    color: Rgba8,
    entity: u32,
    structure: u32,
) -> LabelGpu {
    LabelGpu {
        start_size: start.extend(width).to_array(),
        end_offset: end.extend(0.0).to_array(),
        color: color.to_f32(),
        metadata: [entity, structure, 1, 0],
    }
}

pub(super) fn marker_record(
    anchor: Vec3,
    radius: f32,
    shape: pdviewx_core::MarkerShape,
    color: Rgba8,
    entity: u32,
    structure: u32,
) -> LabelGpu {
    let shape = match shape {
        pdviewx_core::MarkerShape::Circle => 0,
        pdviewx_core::MarkerShape::Diamond => 1,
        pdviewx_core::MarkerShape::Crosshair => 2,
    };
    LabelGpu {
        start_size: anchor.extend(radius).to_array(),
        end_offset: [0.0; 4],
        color: color.to_f32(),
        metadata: [entity, structure, 2, shape],
    }
}

pub(super) fn glyph_bits(value: char) -> u64 {
    glyph_rows(value)
        .iter()
        .enumerate()
        .fold(0u64, |bits, (row, value)| {
            bits | (u64::from(*value) << (row * 5))
        })
}

fn glyph_rows(value: char) -> [u8; 7] {
    match value.to_ascii_uppercase() {
        'A' | 'Å' => [14, 17, 17, 31, 17, 17, 17],
        'B' => [30, 17, 17, 30, 17, 17, 30],
        'C' => [14, 17, 16, 16, 16, 17, 14],
        'D' => [30, 17, 17, 17, 17, 17, 30],
        'E' => [31, 16, 16, 30, 16, 16, 31],
        'F' => [31, 16, 16, 30, 16, 16, 16],
        'G' => [14, 17, 16, 23, 17, 17, 15],
        'H' => [17, 17, 17, 31, 17, 17, 17],
        'I' => [14, 4, 4, 4, 4, 4, 14],
        'J' => [7, 2, 2, 2, 18, 18, 12],
        'K' => [17, 18, 20, 24, 20, 18, 17],
        'L' => [16, 16, 16, 16, 16, 16, 31],
        'M' => [17, 27, 21, 21, 17, 17, 17],
        'N' => [17, 25, 21, 19, 17, 17, 17],
        'O' => [14, 17, 17, 17, 17, 17, 14],
        'P' => [30, 17, 17, 30, 16, 16, 16],
        'Q' => [14, 17, 17, 17, 21, 18, 13],
        'R' => [30, 17, 17, 30, 20, 18, 17],
        'S' => [15, 16, 16, 14, 1, 1, 30],
        'T' => [31, 4, 4, 4, 4, 4, 4],
        'U' => [17, 17, 17, 17, 17, 17, 14],
        'V' => [17, 17, 17, 17, 17, 10, 4],
        'W' => [17, 17, 17, 21, 21, 21, 10],
        'X' => [17, 17, 10, 4, 10, 17, 17],
        'Y' => [17, 17, 10, 4, 4, 4, 4],
        'Z' => [31, 1, 2, 4, 8, 16, 31],
        '0' => [14, 17, 19, 21, 25, 17, 14],
        '1' => [4, 12, 4, 4, 4, 4, 14],
        '2' => [14, 17, 1, 2, 4, 8, 31],
        '3' => [30, 1, 1, 14, 1, 1, 30],
        '4' => [2, 6, 10, 18, 31, 2, 2],
        '5' => [31, 16, 16, 30, 1, 1, 30],
        '6' => [14, 16, 16, 30, 17, 17, 14],
        '7' => [31, 1, 2, 4, 8, 8, 8],
        '8' => [14, 17, 17, 14, 17, 17, 14],
        '9' => [14, 17, 17, 15, 1, 1, 14],
        '-' => [0, 0, 0, 31, 0, 0, 0],
        '.' => [0, 0, 0, 0, 0, 12, 12],
        ':' => [0, 12, 12, 0, 12, 12, 0],
        '/' => [1, 2, 2, 4, 8, 8, 16],
        '°' => [6, 9, 9, 6, 0, 0, 0],
        ' ' => [0; 7],
        _ => [14, 17, 1, 2, 4, 0, 4],
    }
}
