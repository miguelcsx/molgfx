//! Scene-linear HDR capture and native half-float export.

use super::{EffectiveQuality, Renderer};
use crate::{Error, Scene};
use num_traits::ToPrimitive as _;
use std::io::Write as _;

/// Completed scene-linear RGBA16F exposure, before presentation processing.
///
/// Bloom, exposure, tone mapping, display conversion and screen overlays are
/// not baked into the pixels. Export preserves highlights above one instead
/// of clipping them to the display range.
#[derive(Debug)]
pub struct HdrImage(pub(super) molgfx_render::HdrImage);

impl HdrImage {
    /// Physical settings and observed completion for this image.
    #[must_use]
    pub const fn quality(&self) -> &EffectiveQuality {
        self.0.quality()
    }

    /// Pixel width.
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.0.width()
    }

    /// Pixel height.
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.0.height()
    }

    /// Tightly packed row-major RGBA words, little-endian IEEE binary16.
    #[must_use]
    pub fn rgba16f(&self) -> &[u8] {
        self.0.rgba16f()
    }

    /// Encodes deterministic, uncompressed half-float RGBA `OpenEXR`.
    ///
    /// # Errors
    ///
    /// Returns an image encoding error for an invalid pixel layout.
    pub fn exr_bytes(&self) -> Result<Vec<u8>, Error> {
        self.0.exr_bytes().map_err(Error::from)
    }

    /// Streams `OpenEXR` to disk without duplicating the complete pixel buffer.
    ///
    /// The caller supplies the filename; this method always writes `OpenEXR`.
    ///
    /// # Errors
    ///
    /// Returns an encoding or filesystem error, including a failed final flush.
    pub fn save_exr(&self, path: impl AsRef<std::path::Path>) -> Result<(), Error> {
        let file = std::fs::File::create(path)?;
        let mut writer = std::io::BufWriter::new(file);
        self.0.write_exr(&mut writer)?;
        writer.flush()?;
        Ok(())
    }
}

impl Renderer {
    /// Renders a converged scene-linear HDR image with the scene's framing camera.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid target or a renderer/device failure.
    pub fn render_hdr_image(&mut self, scene: &Scene, size: (u32, u32)) -> Result<HdrImage, Error> {
        if size.0 == 0 || size.1 == 0 {
            return Err(Error::InvalidSpec(
                "image width and height must be non-zero".to_owned(),
            ));
        }
        let (Some(width), Some(height)) = (size.0.to_f32(), size.1.to_f32()) else {
            return Err(Error::InvalidSpec(
                "image size cannot be represented".to_owned(),
            ));
        };
        let camera = scene.framing_camera(width / height);
        self.render_hdr_image_with_camera(scene, &camera, size)
    }

    /// Renders a converged scene-linear HDR image with an explicit camera.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid target or a renderer/device failure.
    pub fn render_hdr_image_with_camera(
        &mut self,
        scene: &Scene,
        camera: &molgfx_math::Camera,
        size: (u32, u32),
    ) -> Result<HdrImage, Error> {
        let image = self
            .inner
            .render_hdr_image(
                scene.resolved(),
                camera,
                molgfx_render::ImageConfig {
                    width: size.0,
                    height: size.1,
                },
            )
            .map(HdrImage)
            .map_err(Error::from)?;
        self.pick_source_id = Some(scene.resolved().cache_identity());
        Ok(image)
    }
}
