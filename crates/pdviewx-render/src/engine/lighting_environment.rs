//! Declarative image-based approximation and bounded key/fill light rig.
//!
//! This is presentation state, not biological context. Reflections and ambient
//! response can match the caller's compositing environment without coupling
//! them to the fallback backdrop or to a named graphics backend.

use pdviewx_math::{Rgba8, Vec3};
use serde::{Deserialize, Serialize};

/// Scene-linear lighting sampled by molecular materials.
#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub struct LightingEnvironment {
    /// Upper-hemisphere environment colour in display sRGB.
    pub zenith: Rgba8,
    /// Grazing environment colour in display sRGB.
    pub horizon: Rgba8,
    /// Lower-hemisphere environment colour in display sRGB.
    pub ground: Rgba8,
    /// Grazing-angle separation colour in display sRGB.
    pub rim_color: Rgba8,
    /// Broad key-light colour in display sRGB.
    pub key_color: Rgba8,
    /// Broad fill-light colour in display sRGB.
    pub fill_color: Rgba8,
    /// View-space direction toward the key light.
    pub key_direction: Vec3,
    /// View-space direction toward the fill light.
    pub fill_direction: Vec3,
    /// Diffuse environment multiplier in `[0, 4]`.
    pub diffuse_strength: f32,
    /// Reflected environment multiplier in `[0, 4]`.
    pub specular_strength: f32,
    /// Grazing rim multiplier in `[0, 2]`.
    pub rim_strength: f32,
    /// Key-light multiplier in `[0, 8]`.
    pub key_strength: f32,
    /// Fill-light multiplier in `[0, 8]`.
    pub fill_strength: f32,
    /// Apparent key-light angular radius in `[0, 0.5]`; controls penumbra.
    pub key_angular_radius: f32,
    /// Direct-light shadow attenuation in `[0, 1]`.
    pub shadow_strength: f32,
}

impl LightingEnvironment {
    /// Balanced neutral illumination for quantitative inspection.
    #[must_use]
    pub const fn neutral() -> Self {
        Self {
            zenith: Rgba8::opaque(170, 184, 204),
            horizon: Rgba8::opaque(118, 126, 138),
            ground: Rgba8::opaque(72, 66, 60),
            rim_color: Rgba8::opaque(132, 156, 202),
            key_color: Rgba8::opaque(255, 244, 230),
            fill_color: Rgba8::opaque(151, 181, 224),
            key_direction: Vec3::new(-0.42, 0.58, 0.70),
            fill_direction: Vec3::new(0.78, -0.12, 0.50),
            diffuse_strength: 0.72,
            specular_strength: 0.58,
            rim_strength: 0.22,
            key_strength: 1.55,
            fill_strength: 0.28,
            key_angular_radius: 0.16,
            shadow_strength: 0.48,
        }
    }

    /// Soft documentary rig with stronger shape separation.
    #[must_use]
    pub const fn documentary() -> Self {
        Self {
            // A specimen lit against a dark sweep sits in a dark environment:
            // dimming the ambient lets the key shape the form and keeps albedo
            // saturated instead of washing it toward the ambient's own colour.
            // Matched to the bright sweep. A specimen in a lightbox sits in a
            // bright environment, so the ambient is high and the key only
            // shapes it; a dark ambient under a bright backdrop would read as
            // a cut-out pasted onto the frame.
            zenith: Rgba8::opaque(226, 232, 240),
            horizon: Rgba8::opaque(188, 194, 202),
            ground: Rgba8::opaque(150, 146, 141),
            fill_color: Rgba8::opaque(206, 214, 226),
            rim_color: Rgba8::opaque(120, 132, 152),
            diffuse_strength: 0.62,
            specular_strength: 0.72,
            rim_strength: 0.16,
            key_strength: 1.35,
            fill_strength: 0.30,
            key_angular_radius: 0.20,
            shadow_strength: 0.66,
            ..Self::neutral()
        }
    }

    pub(crate) fn sanitize(self) -> Self {
        let neutral = Self::neutral();
        Self {
            key_direction: direction(self.key_direction, neutral.key_direction),
            fill_direction: direction(self.fill_direction, neutral.fill_direction),
            diffuse_strength: bounded(self.diffuse_strength, 4.0, neutral.diffuse_strength),
            specular_strength: bounded(self.specular_strength, 4.0, neutral.specular_strength),
            rim_strength: bounded(self.rim_strength, 2.0, neutral.rim_strength),
            key_strength: bounded(self.key_strength, 8.0, neutral.key_strength),
            fill_strength: bounded(self.fill_strength, 8.0, neutral.fill_strength),
            key_angular_radius: bounded(self.key_angular_radius, 0.5, neutral.key_angular_radius),
            shadow_strength: bounded(self.shadow_strength, 1.0, neutral.shadow_strength),
            ..self
        }
    }

    pub(crate) fn blend(self, other: Self, weight: f32) -> Self {
        let weight = unit(weight);
        let choose = |current, next| if weight >= 0.5 { next } else { current };
        Self {
            zenith: choose(self.zenith, other.zenith),
            horizon: choose(self.horizon, other.horizon),
            ground: choose(self.ground, other.ground),
            rim_color: choose(self.rim_color, other.rim_color),
            key_color: choose(self.key_color, other.key_color),
            fill_color: choose(self.fill_color, other.fill_color),
            key_direction: self.key_direction.lerp(other.key_direction, weight),
            fill_direction: self.fill_direction.lerp(other.fill_direction, weight),
            diffuse_strength: lerp(self.diffuse_strength, other.diffuse_strength, weight),
            specular_strength: lerp(self.specular_strength, other.specular_strength, weight),
            rim_strength: lerp(self.rim_strength, other.rim_strength, weight),
            key_strength: lerp(self.key_strength, other.key_strength, weight),
            fill_strength: lerp(self.fill_strength, other.fill_strength, weight),
            key_angular_radius: lerp(self.key_angular_radius, other.key_angular_radius, weight),
            shadow_strength: lerp(self.shadow_strength, other.shadow_strength, weight),
        }
        .sanitize()
    }

    pub(crate) fn packed(self) -> [[f32; 4]; 8] {
        let lighting = self.sanitize();
        let lane = |color: Rgba8, strength| {
            let rgb = super::backdrop::linear_rgb(color);
            [rgb[0], rgb[1], rgb[2], strength]
        };
        [
            lane(lighting.zenith, lighting.diffuse_strength),
            lane(lighting.horizon, lighting.specular_strength),
            lane(lighting.ground, lighting.rim_strength),
            lane(lighting.rim_color, lighting.shadow_strength),
            lighting
                .key_direction
                .extend(lighting.key_strength)
                .to_array(),
            lane(lighting.key_color, lighting.key_angular_radius),
            lighting
                .fill_direction
                .extend(lighting.fill_strength)
                .to_array(),
            lane(lighting.fill_color, 0.0),
        ]
    }
}

impl Default for LightingEnvironment {
    fn default() -> Self {
        Self::documentary()
    }
}

fn direction(value: Vec3, fallback: Vec3) -> Vec3 {
    if value.is_finite() && value.length_squared() > 1.0e-8 {
        value.normalize()
    } else {
        fallback.normalize()
    }
}

fn bounded(value: f32, maximum: f32, fallback: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, maximum)
    } else {
        fallback
    }
}

fn unit(value: f32) -> f32 {
    bounded(value, 1.0, 0.0)
}

fn lerp(from: f32, to: f32, weight: f32) -> f32 {
    from + (to - from) * weight
}
