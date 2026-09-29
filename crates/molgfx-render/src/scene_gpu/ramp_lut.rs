//! Scalar colour ramps baked into fixed lookup tables.
//!
//! A ramp has up to sixteen stops, but the shader resolves it with the same
//! two table reads whatever the count: the ramp is sampled once on the host at
//! 256 evenly spaced values across its domain and the packed colours are
//! uploaded. Rebuilding is `O(256 log stops)` and happens only when a
//! representation's colour or overlay changes, never per frame or per atom.

use molgfx_core::ScalarRamp;
use molgfx_math::Rgba8;

/// Table entries; matches `RAMP_LUT_SIZE` in the shader.
pub(crate) const RAMP_LUT_SIZE: usize = 256;
/// Packed colours per uniform element.
const LANES: usize = 4;

/// A baked ramp, byte-identical to the shader's `RampLut`.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct RampLut {
    /// First stop value, table steps per unit, the packed missing colour bits,
    /// and a reserved lane.
    domain: [f32; 4],
    colors: [[u32; LANES]; RAMP_LUT_SIZE / LANES],
}

impl RampLut {
    /// Bakes `ramp`; `missing` is carried for values that are not finite.
    pub(crate) fn new(ramp: &ScalarRamp, missing: Rgba8) -> Self {
        let [first, last] = ramp.domain();
        let span = last - first;
        let mut colors = [[0_u32; LANES]; RAMP_LUT_SIZE / LANES];
        for index in 0..RAMP_LUT_SIZE {
            let fraction = u16::try_from(index).map_or(1.0, f32::from)
                / u16::try_from(RAMP_LUT_SIZE - 1).map_or(1.0, f32::from);
            let color = ramp.sample(first + span * fraction, missing);
            colors[index / LANES][index % LANES] = pack(color);
        }
        let steps = u16::try_from(RAMP_LUT_SIZE - 1).map_or(1.0, f32::from);
        Self {
            domain: [first, steps / span, f32::from_bits(pack(missing)), 0.0],
            colors,
        }
    }

    /// An inert table for a representation that has no ramp.
    pub(crate) fn disabled() -> Self {
        bytemuck::Zeroable::zeroed()
    }

    /// One entry as the shader would read it, for tests.
    #[cfg(test)]
    pub(crate) fn entry(&self, index: usize) -> u32 {
        self.colors[index / LANES][index % LANES]
    }

    /// First stop value and table steps per unit, for tests.
    #[cfg(test)]
    pub(crate) fn domain_probe(&self) -> [f32; 2] {
        [self.domain[0], self.domain[1]]
    }
}

/// One colour as the packed word the shader unpacks.
const fn pack(color: Rgba8) -> u32 {
    u32::from_le_bytes([color.r, color.g, color.b, color.a])
}

#[cfg(test)]
#[path = "ramp_lut_tests.rs"]
mod tests;
