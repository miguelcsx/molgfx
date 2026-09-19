//! Minimal deterministic `OpenEXR` scanline encoder for native RGBA16F data.

use super::HdrImage;
use crate::error::RenderError;
use std::io::Write;

const MAGIC: u32 = 20_000_630;
const VERSION: u32 = 2;
const HEADER_CAPACITY: usize = 512;
const CHANNEL_BYTES: usize = 2;
const CHANNEL_COUNT: usize = 4;
const BLOCK_HEADER_BYTES: usize = 8;
const OFFSET_BYTES: usize = 8;

pub(super) fn write(image: &HdrImage, mut writer: impl Write) -> Result<(), RenderError> {
    let width = usize::try_from(image.width).map_err(|_| encoding_overflow())?;
    let height = usize::try_from(image.height).map_err(|_| encoding_overflow())?;
    let row_bytes = width
        .checked_mul(CHANNEL_COUNT * CHANNEL_BYTES)
        .ok_or_else(encoding_overflow)?;
    let pixel_bytes = row_bytes
        .checked_mul(height)
        .ok_or_else(encoding_overflow)?;
    if width == 0 || height == 0 || image.rgba16f.len() != pixel_bytes {
        return Err(RenderError::ImageEncoding {
            summary: "RGBA16F pixel buffer length does not match image dimensions".to_owned(),
        });
    }
    let max_x = i32::try_from(image.width - 1).map_err(|_| encoding_overflow())?;
    let max_y = i32::try_from(image.height - 1).map_err(|_| encoding_overflow())?;
    let block_bytes = row_bytes
        .checked_add(BLOCK_HEADER_BYTES)
        .ok_or_else(encoding_overflow)?;
    let table_bytes = height
        .checked_mul(OFFSET_BYTES)
        .ok_or_else(encoding_overflow)?;
    let mut header = Vec::with_capacity(HEADER_CAPACITY);
    write_header(&mut header, max_x, max_y);
    write_all(&mut writer, &header)?;
    let first_block = header
        .len()
        .checked_add(table_bytes)
        .ok_or_else(encoding_overflow)?;
    for row in 0..height {
        let offset = first_block
            .checked_add(block_bytes.checked_mul(row).ok_or_else(encoding_overflow)?)
            .ok_or_else(encoding_overflow)?;
        write_all(
            &mut writer,
            &u64::try_from(offset)
                .map_err(|_| encoding_overflow())?
                .to_le_bytes(),
        )?;
    }
    let encoded_row_bytes = i32::try_from(row_bytes).map_err(|_| encoding_overflow())?;
    let mut planar = Vec::with_capacity(row_bytes);
    for (row_index, row) in image.rgba16f.chunks_exact(row_bytes).enumerate() {
        write_all(
            &mut writer,
            &i32::try_from(row_index)
                .map_err(|_| encoding_overflow())?
                .to_le_bytes(),
        )?;
        write_all(&mut writer, &encoded_row_bytes.to_le_bytes())?;
        planar.clear();
        for component in [6_usize, 4, 2, 0] {
            for pixel in row.chunks_exact(CHANNEL_COUNT * CHANNEL_BYTES) {
                planar.extend_from_slice(&pixel[component..component + CHANNEL_BYTES]);
            }
        }
        write_all(&mut writer, &planar)?;
    }
    Ok(())
}

fn write_header(output: &mut Vec<u8>, max_x: i32, max_y: i32) {
    write_u32(output, MAGIC);
    write_u32(output, VERSION);
    write_channels(output);
    write_chromaticities(output);
    write_attribute(output, "compression", "compression", &[0]);
    write_box(output, "dataWindow", max_x, max_y);
    write_box(output, "displayWindow", max_x, max_y);
    write_attribute(output, "lineOrder", "lineOrder", &[0]);
    write_attribute(output, "pixelAspectRatio", "float", &1.0_f32.to_le_bytes());
    let mut center = [0_u8; 8];
    center[0..4].copy_from_slice(&0.0_f32.to_le_bytes());
    center[4..8].copy_from_slice(&0.0_f32.to_le_bytes());
    write_attribute(output, "screenWindowCenter", "v2f", &center);
    write_attribute(output, "screenWindowWidth", "float", &1.0_f32.to_le_bytes());
    output.push(0);
}

fn write_channels(output: &mut Vec<u8>) {
    write_attribute_header(output, "channels", "chlist", 73);
    for &name in b"ABGR" {
        output.extend_from_slice(&[name, 0]);
        write_i32(output, 1);
        output.extend_from_slice(&[0, 0, 0, 0]);
        write_i32(output, 1);
        write_i32(output, 1);
    }
    output.push(0);
}

fn write_chromaticities(output: &mut Vec<u8>) {
    let primaries = [
        0.6400_f32, 0.3300, 0.3000, 0.6000, 0.1500, 0.0600, 0.3127, 0.3290,
    ];
    write_attribute_header(output, "chromaticities", "chromaticities", 32);
    for value in primaries {
        output.extend_from_slice(&value.to_le_bytes());
    }
}

fn write_box(output: &mut Vec<u8>, name: &str, max_x: i32, max_y: i32) {
    let mut value = [0_u8; 16];
    value[8..12].copy_from_slice(&max_x.to_le_bytes());
    value[12..16].copy_from_slice(&max_y.to_le_bytes());
    write_attribute(output, name, "box2i", &value);
}

fn write_attribute(output: &mut Vec<u8>, name: &str, kind: &str, value: &[u8]) {
    let size = u32::try_from(value.len()).map_or(u32::MAX, |size| size);
    write_attribute_header(output, name, kind, size);
    output.extend_from_slice(value);
}

fn write_attribute_header(output: &mut Vec<u8>, name: &str, kind: &str, size: u32) {
    output.extend_from_slice(name.as_bytes());
    output.push(0);
    output.extend_from_slice(kind.as_bytes());
    output.push(0);
    write_u32(output, size);
}

fn write_i32(output: &mut Vec<u8>, value: i32) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn write_u32(output: &mut Vec<u8>, value: u32) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn encoding_overflow() -> RenderError {
    RenderError::ImageEncoding {
        summary: "OpenEXR layout exceeds supported offsets".to_owned(),
    }
}

fn write_all(writer: &mut impl Write, bytes: &[u8]) -> Result<(), RenderError> {
    writer
        .write_all(bytes)
        .map_err(|error| RenderError::ImageEncoding {
            summary: format!("OpenEXR write failed: {error}"),
        })
}

#[cfg(test)]
#[path = "exr_tests.rs"]
mod tests;
