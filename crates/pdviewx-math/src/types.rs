//! Core math types, re-exported from glam, plus the packed color used in
//! GPU-facing records.
//!
//! Everything here is plain data, byte-castable for direct GPU upload.

pub use glam::{Mat3, Mat4, Quat, Vec2, Vec3, Vec4};
use serde::{Deserialize, Serialize};

/// A color packed as four 8-bit channels, in memory order red, green, blue,
/// alpha. Exactly four bytes, so a per-atom color column stays compact and
/// uploads without conversion.
#[repr(C)]
#[derive(
    Clone,
    Copy,
    PartialEq,
    Eq,
    Debug,
    Default,
    Serialize,
    Deserialize,
    bytemuck::Pod,
    bytemuck::Zeroable,
)]
pub struct Rgba8 {
    /// Red channel, 0–255.
    pub r: u8,
    /// Green channel, 0–255.
    pub g: u8,
    /// Blue channel, 0–255.
    pub b: u8,
    /// Alpha channel, 0–255; 255 is opaque.
    pub a: u8,
}

impl Rgba8 {
    /// Opaque white.
    pub const WHITE: Self = Self::new(255, 255, 255, 255);

    /// Builds a color from four channel values.
    #[must_use]
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Builds an opaque color from three channel values.
    #[must_use]
    pub const fn opaque(r: u8, g: u8, b: u8) -> Self {
        Self::new(r, g, b, 255)
    }

    /// Converts to normalized floating-point channels in [0, 1].
    #[must_use]
    pub fn to_f32(self) -> [f32; 4] {
        [
            f32::from(self.r) / 255.0,
            f32::from(self.g) / 255.0,
            f32::from(self.b) / 255.0,
            f32::from(self.a) / 255.0,
        ]
    }
}
