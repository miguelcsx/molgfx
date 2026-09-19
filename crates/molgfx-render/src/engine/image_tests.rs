use super::{Image, ImageConfig, ImageLayout, ImagePurpose};
use crate::engine::tests::{camera, engine};
use molgfx_core::Scene;

#[test]
fn publication_png_round_trips_rgba_pixels() {
    let image = Image {
        width: 2,
        height: 1,
        pixels: vec![12, 34, 56, 255, 200, 180, 160, 128],
    };
    let bytes = match image.png_bytes() {
        Ok(bytes) => bytes,
        Err(error) => panic!("PNG encoding succeeds: {error}"),
    };
    let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    let mut reader = match decoder.read_info() {
        Ok(reader) => reader,
        Err(error) => panic!("PNG header is readable: {error}"),
    };
    let Some(size) = reader.output_buffer_size() else {
        panic!("PNG decoder reports an output buffer")
    };
    let mut pixels = vec![0; size];
    let output = match reader.next_frame(&mut pixels) {
        Ok(output) => output,
        Err(error) => panic!("PNG pixels are readable: {error}"),
    };
    assert_eq!(&pixels[..output.buffer_size()], image.pixels.as_slice());
}

#[test]
fn malformed_publication_image_is_rejected_before_encoding() {
    let image = Image {
        width: 2,
        height: 1,
        pixels: vec![0; 3],
    };
    assert!(image.png_bytes().is_err());
}

#[test]
fn padded_rows_are_compacted_without_reallocating_the_frame() {
    let layout = match ImageLayout::new(
        ImageConfig {
            width: 3,
            height: 2,
        },
        4,
    ) {
        Ok(layout) => layout,
        Err(error) => panic!("image layout is valid: {error}"),
    };
    let buffer_size = match usize::try_from(layout.buffer_size) {
        Ok(size) => size,
        Err(error) => panic!("image buffer fits host address space: {error}"),
    };
    let mut mapped = vec![0xee; buffer_size];
    mapped[..layout.row].copy_from_slice(&(0_u8..12).collect::<Vec<_>>());
    let second = layout.padded_row as usize;
    mapped[second..second + layout.row].copy_from_slice(&(12_u8..24).collect::<Vec<_>>());
    let allocation = mapped.as_ptr();

    let compact = match layout.unpack(mapped) {
        Ok(compact) => compact,
        Err(error) => panic!("padded rows unpack: {error}"),
    };

    assert_eq!(compact.as_ptr(), allocation);
    assert_eq!(compact, (0_u8..24).collect::<Vec<_>>());
}

#[test]
fn sequence_frames_preserve_history_while_publication_stills_reset_it() {
    let mut engine = engine();
    let scene = Scene::new();
    let config = ImageConfig {
        width: 32,
        height: 24,
    };
    assert!(
        engine
            .render_image_to_buffer(&scene, &camera(), config, ImagePurpose::SequenceFrame)
            .is_ok()
    );
    assert_eq!(engine.temporal.write_index(), 0);
    assert_eq!(engine.temporal_scene_identity, Some(scene.cache_identity()));
    assert!(
        engine
            .render_image_to_buffer(&scene, &camera(), config, ImagePurpose::SequenceFrame)
            .is_ok()
    );
    assert_eq!(engine.temporal.write_index(), 1);
    assert!(
        engine
            .render_image_to_buffer(&scene, &camera(), config, ImagePurpose::Publication)
            .is_ok()
    );
    assert_eq!(engine.temporal_scene_identity, None);
}
