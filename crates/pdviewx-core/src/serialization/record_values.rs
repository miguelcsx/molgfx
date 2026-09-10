//! Small stable value encodings shared by manifest record builders.

use crate::VolumeRendering;

pub(super) const fn volume_rendering(value: VolumeRendering) -> &'static str {
    match value {
        VolumeRendering::Direct => "direct",
        VolumeRendering::Isosurface => "isosurface",
        VolumeRendering::Medium => "medium",
        VolumeRendering::Slice => "slice",
        VolumeRendering::LiquidSurface => "liquid_surface",
    }
}
