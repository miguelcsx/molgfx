//! Presentation-format selection kept separate from graph construction.

use crate::engine::EngineConfig;
use molgfx_gpu::{Device, Surface as _, SurfaceConfig, TextureFormat};

pub(in crate::engine) fn configure<D: Device>(
    device: &D,
    surface: &mut Option<D::Surface>,
    config: &EngineConfig,
) -> TextureFormat {
    let Some(surface) = surface else {
        return TextureFormat::Rgba8Unorm;
    };
    surface.configure(
        device,
        &SurfaceConfig {
            width: config.width,
            height: config.height,
            format: TextureFormat::Bgra8Unorm,
        },
    );
    surface.config().format
}
