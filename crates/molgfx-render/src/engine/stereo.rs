//! Parallel stereo eye exposures and O(pixels) presentation composition.
use super::Image;
#[cfg(not(target_arch = "wasm32"))]
use super::{Engine, ImageConfig};
use crate::RenderError;
#[cfg(not(target_arch = "wasm32"))]
use molgfx_core::Scene;
#[cfg(not(target_arch = "wasm32"))]
use molgfx_gpu::Device;
#[cfg(not(target_arch = "wasm32"))]
use molgfx_math::Camera;

mod pixels;

/// Layout of already rendered left/right eye images in physical pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StereoLayout {
    /// Two full-width eyes, left first.
    SideBySide,
    /// Two full-width eyes, right first for cross-eyed viewing.
    CrossEyed,
    /// Horizontally compressed eyes in one original-width image.
    HdSideBySide,
    /// Alternating pixels, left eye at even row-plus-column parity.
    Checkerboard,
    /// Alternating columns, left eye at even columns.
    Columns,
    /// Alternating rows, left eye at even rows.
    Rows,
    /// Left red channel and right green/blue channels.
    RedCyan,
    /// Left red channel and right blue channel.
    RedBlue,
    /// Left red channel and right green channel.
    RedGreen,
    /// Left eye alone.
    Left,
    /// Right eye alone.
    Right,
}

/// Separate eye exposures, also suitable for host-owned quad-buffer presentation.
#[derive(Debug)]
pub struct StereoImages {
    /// Complete left-eye exposure.
    pub left: Image,
    /// Complete right-eye exposure.
    pub right: Image,
}

impl StereoImages {
    /// Composes an image layout without resampling geometry or applying grading twice.
    ///
    /// # Errors
    /// Rejects malformed/mismatched eye images or an overflowing output extent.
    pub fn compose(&self, layout: StereoLayout) -> Result<Image, RenderError> {
        self.left.compose_stereo(&self.right, layout)
    }
}

impl Image {
    /// Composes a matching right-eye image into a stereo display layout.
    ///
    /// # Errors
    /// Rejects invalid eye buffers, mismatched quality, dimensions or overflowing output.
    pub fn compose_stereo(&self, right: &Self, layout: StereoLayout) -> Result<Image, RenderError> {
        if self.width != right.width || self.height != right.height {
            return Err(RenderError::InvalidStereo {
                reason: "eye extents must match",
            });
        }
        let (width, pixels) =
            pixels::compose(&self.pixels, &right.pixels, self.width, self.height, layout)?;
        let mut quality = match layout {
            StereoLayout::Left => self.quality,
            StereoLayout::Right => right.quality,
            _ if self.quality == right.quality => self.quality,
            _ => {
                return Err(RenderError::InvalidStereo {
                    reason: "eye quality must match",
                });
            }
        };
        quality.extent = [width, self.height];
        Ok(Image {
            width,
            height: self.height,
            pixels,
            quality,
        })
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl<D: Device> Engine<D> {
    /// Renders independent, converged eye exposures using parallel off-axis views.
    /// Use each image directly for a host stereo target or compose a display layout.
    ///
    /// # Errors
    /// Returns stereo validation, image extent or rendering errors.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn render_stereo_images(
        &mut self,
        scene: &Scene,
        camera: &Camera,
        size: ImageConfig,
        separation: f32,
        convergence: f32,
    ) -> Result<StereoImages, RenderError> {
        let [left, right] =
            camera
                .stereo_eyes(separation, convergence)
                .ok_or(RenderError::InvalidStereo {
                    reason: "invalid eye separation, convergence or camera",
                })?;
        Ok(StereoImages {
            left: self.render_image(scene, &left, size)?,
            right: self.render_image(scene, &right, size)?,
        })
    }
}
