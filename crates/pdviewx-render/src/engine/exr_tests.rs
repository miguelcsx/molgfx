use super::{MAGIC, VERSION};
use crate::engine::HdrImage;

#[test]
fn exr_preserves_native_half_channels_without_a_colour_curve() {
    let image = HdrImage {
        width: 1,
        height: 1,
        rgba16f: vec![0x00, 0x3c, 0x00, 0x40, 0x00, 0x42, 0x00, 0x38],
    };
    let bytes = encoded(&image);
    assert_eq!(&bytes[0..4], MAGIC.to_le_bytes());
    assert_eq!(&bytes[4..8], VERSION.to_le_bytes());
    let header_end = parse_header_end(&bytes);
    let block_offset = read_u64(&bytes, header_end);
    assert_eq!(block_offset, u64::try_from(header_end + 8).unwrap_or(0));
    let block = usize::try_from(block_offset).unwrap_or(0);
    assert_eq!(read_i32(&bytes, block), 0);
    assert_eq!(read_i32(&bytes, block + 4), 8);
    assert_eq!(
        &bytes[block + 8..block + 16],
        &[0x00, 0x38, 0x00, 0x42, 0x00, 0x40, 0x00, 0x3c],
        "OpenEXR scanlines are planar A, B, G, R"
    );
}

#[test]
fn exr_rejects_a_malformed_half_pixel_buffer() {
    let image = HdrImage {
        width: 2,
        height: 1,
        rgba16f: vec![0; 8],
    };
    assert!(super::write(&image, Vec::new()).is_err());
}

#[test]
fn streamed_exr_is_byte_deterministic() {
    let image = HdrImage {
        width: 2,
        height: 2,
        rgba16f: (0_u8..32).collect(),
    };
    assert_eq!(encoded(&image), encoded(&image));
}

fn encoded(image: &HdrImage) -> Vec<u8> {
    let mut output = Vec::new();
    if let Err(error) = super::write(image, &mut output) {
        panic!("streamed EXR encodes: {error}")
    }
    output
}

fn parse_header_end(bytes: &[u8]) -> usize {
    let mut cursor = 8;
    loop {
        let Some(&byte) = bytes.get(cursor) else {
            panic!("EXR header terminates")
        };
        if byte == 0 {
            return cursor + 1;
        }
        cursor = after_c_string(bytes, cursor);
        cursor = after_c_string(bytes, cursor);
        let size = usize::try_from(read_u32(bytes, cursor)).unwrap_or(0);
        cursor = cursor.saturating_add(4).saturating_add(size);
    }
}

fn after_c_string(bytes: &[u8], start: usize) -> usize {
    let Some(relative) = bytes
        .get(start..)
        .and_then(|tail| tail.iter().position(|byte| *byte == 0))
    else {
        panic!("EXR string terminates")
    };
    start + relative + 1
}

fn read_i32(bytes: &[u8], offset: usize) -> i32 {
    i32::from_le_bytes(read_array(bytes, offset))
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(read_array(bytes, offset))
}

fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    let Some(slice) = bytes.get(offset..offset + 8) else {
        panic!("EXR u64 exists")
    };
    let mut value = [0; 8];
    value.copy_from_slice(slice);
    u64::from_le_bytes(value)
}

fn read_array(bytes: &[u8], offset: usize) -> [u8; 4] {
    let Some(slice) = bytes.get(offset..offset + 4) else {
        panic!("EXR u32 exists")
    };
    let mut value = [0; 4];
    value.copy_from_slice(slice);
    value
}
