use super::HdrImage;
use crate::engine::ImageConfig;
use crate::engine::tests::{camera, engine};
use pdviewx_core::Scene;

#[test]
fn hdr_capture_returns_tightly_packed_native_half_pixels() {
    let mut engine = engine();
    let image = match engine.render_hdr_image(
        &Scene::new(),
        &camera(),
        ImageConfig {
            width: 3,
            height: 2,
        },
    ) {
        Ok(image) => image,
        Err(error) => panic!("HDR image renders: {error}"),
    };
    assert_eq!((image.width, image.height), (3, 2));
    assert_eq!(image.rgba16f.len(), 3 * 2 * 8);
    let Ok(textures) = engine.device.log.textures.lock() else {
        panic!("texture log locks")
    };
    assert!(
        !textures.contains(&"off-screen image"),
        "HDR capture reuses the graph texture"
    );
}

#[test]
fn hdr_capture_rejects_zero_sized_images() {
    let mut engine = engine();
    let result = engine.render_hdr_image(
        &Scene::new(),
        &camera(),
        ImageConfig {
            width: 0,
            height: 2,
        },
    );
    assert!(result.is_err());
}

#[test]
fn asynchronous_hdr_capture_uses_the_same_packed_layout() {
    let mut engine = engine();
    let rendered = pollster::block_on(engine.render_hdr_image_async(
        &Scene::new(),
        &camera(),
        ImageConfig {
            width: 3,
            height: 2,
        },
    ));
    let image = match rendered {
        Ok(image) => image,
        Err(error) => panic!("asynchronous HDR image renders: {error}"),
    };
    assert_eq!(image.rgba16f.len(), 3 * 2 * 8);
}

#[test]
fn hdr_exr_rejects_dimensions_that_do_not_match_pixels() {
    let image = HdrImage {
        width: 2,
        height: 2,
        rgba16f: Vec::new(),
    };
    assert!(image.write_exr(Vec::new()).is_err());
}
