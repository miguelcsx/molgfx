//! Backend-selected stereo exposures and explicit display layouts.
use super::{Image, Renderer};
use crate::{Camera, Error, Scene};

pub use molgfx_render::StereoLayout;

/// Matching complete eye images for composition or host stereo presentation.
#[derive(Debug)]
pub struct StereoImages {
    /// Left eye image.
    pub left: Image,
    /// Right eye image.
    pub right: Image,
}

impl StereoImages {
    /// Composes the eye images into the requested pixel layout.
    ///
    /// # Errors
    /// Returns invalid extent or image-buffer errors.
    pub fn compose(&self, layout: StereoLayout) -> Result<Image, Error> {
        self.left
            .0
            .compose_stereo(&self.right.0, layout)
            .map(Image)
            .map_err(Error::from)
    }
}

impl Renderer {
    /// Renders converged left/right views with world-unit separation and convergence.
    /// Separate images can be presented through a host-owned quad-buffer window.
    ///
    /// # Errors
    /// Returns camera, extent, rendering or device errors.
    pub fn render_stereo_images(
        &mut self,
        scene: &Scene,
        camera: &Camera,
        size: (u32, u32),
        separation: f32,
        convergence: f32,
    ) -> Result<StereoImages, Error> {
        let images = self.inner.render_stereo_images(
            scene.resolved(),
            camera,
            molgfx_render::ImageConfig {
                width: size.0,
                height: size.1,
            },
            separation,
            convergence,
        )?;
        self.pick_source_id = Some(scene.resolved().cache_identity());
        Ok(StereoImages {
            left: Image(images.left),
            right: Image(images.right),
        })
    }
}
