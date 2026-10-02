//! A rendered image and its encodings.

use super::EffectiveQuality;
use crate::Error;

/// Encoded image returned by off-screen rendering.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug)]
pub struct Image(pub(super) molgfx_render::Image);

#[cfg(not(target_arch = "wasm32"))]
impl Image {
    /// Physical settings and observed completion for this image.
    #[must_use]
    pub const fn quality(&self) -> &EffectiveQuality {
        &self.0.quality
    }

    /// Row-major RGBA8 pixels with no row padding.
    #[must_use]
    pub fn pixels(&self) -> &[u8] {
        &self.0.pixels
    }

    /// Encodes the image as PNG.
    ///
    /// # Errors
    ///
    /// Returns an image encoding error if the pixels cannot be encoded.
    pub fn png_bytes(&self) -> Result<Vec<u8>, Error> {
        self.0.png_bytes().map_err(Error::from)
    }

    /// Writes a PNG image to disk.
    ///
    /// # Errors
    ///
    /// Returns an encoding or filesystem error.
    pub fn save(&self, path: impl AsRef<std::path::Path>) -> Result<(), Error> {
        let file = std::fs::File::create(path)?;
        self.0
            .write_png(std::io::BufWriter::new(file))
            .map_err(Error::from)
    }

    /// Pixel width.
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.0.width
    }

    /// Pixel height.
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.0.height
    }
}
