use super::{Image, ImageConfig, ImageLayout};

fn unrendered_quality() -> crate::EffectiveQuality {
    crate::EffectiveQuality {
        extent: [2, 1],
        tier: crate::QualityTier::High,
        samples_required: 64,
        samples_submitted: 0,
        samples_completed: None,
        surface_spacing_requested: 0.25,
        surface_spacing_effective: None,
        surface_limit: crate::SurfaceLimit::None,
        ribbon_steps_max: 8,
        occlusion_rays_per_sample: 0,
        lighting: crate::LightingEnvironment::neutral(),
        illumination_bounces: 0,
        lod_mode_max: 0,
        progressive: false,
        adaptive: false,
        full_residency: true,
    }
}

#[test]
fn completion_requires_a_fence_and_requested_detail() {
    let mut quality = unrendered_quality();
    quality.samples_submitted = 64;
    assert!(!quality.complete());
    quality.samples_completed = Some(63);
    assert!(!quality.complete());
    quality.samples_completed = Some(64);
    assert!(quality.complete());
    quality.surface_spacing_effective = Some([0.25, 0.5]);
    assert!(!quality.complete());
    quality.surface_spacing_effective = Some([0.25, 0.25]);
    quality.full_residency = false;
    assert!(!quality.complete());
    quality.full_residency = true;
    quality.lod_mode_max = 1;
    assert!(!quality.complete());
    quality.lod_mode_max = 0;
    quality.progressive = true;
    assert!(!quality.complete());
    quality.progressive = false;
    quality.samples_required = 0;
    assert!(!quality.complete());
}

#[test]
fn publication_png_round_trips_rgba_pixels() {
    let image = Image {
        width: 2,
        height: 1,
        pixels: vec![12, 34, 56, 255, 200, 180, 160, 128],
        quality: unrendered_quality(),
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
        quality: unrendered_quality(),
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
