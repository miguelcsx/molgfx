//! The 8-bit sRGB color value.

use molgfx_math::Rgba8;
use serde::{Deserialize, Serialize};

/// Native CPK colour shared by molecular atoms and generic point renderings.
#[must_use]
pub fn element_rgb(atomic_number: u8) -> [u8; 3] {
    let color = molgfx_core::cpk_color(atomic_number);
    [color.r, color.g, color.b]
}

/// A serializable RGBA color.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct Color(pub [u8; 4]);

impl Color {
    /// Opaque RGB color.
    #[must_use]
    pub const fn rgb(red: u8, green: u8, blue: u8) -> Self {
        Self([red, green, blue, 255])
    }

    /// Linear floating-point channels suitable for shader literals.
    #[must_use]
    pub fn to_linear_f32(self) -> [f32; 4] {
        [
            srgb_to_linear(self.0[0]),
            srgb_to_linear(self.0[1]),
            srgb_to_linear(self.0[2]),
            f32::from(self.0[3]) / 255.0,
        ]
    }

    pub(crate) const fn native(self) -> Rgba8 {
        Rgba8::new(self.0[0], self.0[1], self.0[2], self.0[3])
    }
}

pub(super) fn srgb_to_linear(channel: u8) -> f32 {
    let value = f32::from(channel) / 255.0;
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}
