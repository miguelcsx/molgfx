//! The palette and selector block the GPU colour schemes resolve against.
//!
//! Colour used to be resolved on the CPU and baked into each 20-byte instance
//! record, so changing a scheme repacked and re-uploaded every atom. The record
//! now carries the element colour and three palette indices; this block carries
//! the palette and which scheme to apply. Changing a scheme is therefore one
//! fixed-size uniform write.

use molgfx_core::{CATEGORICAL_COLORS, ColorScheme, Representation, SECONDARY_STRUCTURE_COLORS};
use molgfx_gpu::{Device, Queue as _};
use molgfx_math::Rgba8;

/// The colour scheme tags the shader switches on.
///
/// They are shader constants; naming them here is what keeps the selector a
/// single word rather than a set of flags that could disagree.
const SCHEME_ELEMENT: u32 = 0;
const SCHEME_CHAIN: u32 = 1;
const SCHEME_RESIDUE: u32 = 2;
const SCHEME_SECONDARY: u32 = 3;
const SCHEME_PROPERTY: u32 = 4;
const SCHEME_UNIFORM: u32 = 5;

/// The number of palette slots the shader declares.
const PALETTE_SLOTS: usize = 17;
/// Where the ramp stops live.
const RAMP_BASE: usize = 0;
/// The ramp row, which also carries the missing colour packed in its free lane.
const RAMP_ROW: usize = 3;
/// Where the five secondary-structure colours begin.
const SECONDARY_BASE: usize = 4;
/// Where the eight chain colours begin.
///
/// Chains and residues share this block: both reduce to an index into one
/// categorical palette, so they differ only in how the index is derived, not in
/// which colours they name.
const CATEGORICAL_BASE: usize = 9;

/// The palette and selector, byte-identical to the shader's block.
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct ColorUniforms {
    palette: [[f32; 4]; PALETTE_SLOTS],
    /// Scheme tag, the packed uniform colour, the property column's arena
    /// offset and its stride in words.
    selector: [u32; 4],
    /// Appearance domain in `xy` and its opacity response in `zw`.
    ///
    /// A domain that does not increase means no appearance mapping is active,
    /// which is what its own constructor guarantees, so no separate flag is
    /// needed.
    appearance: [f32; 4],
    /// The softness response in `xy` and the missing-value response in `zw`.
    softness: [f32; 4],
}

impl ColorUniforms {
    /// Builds the block for one representation.
    ///
    /// `column` is the colour property column's arena offset and stride, or
    /// zero when no column was planned: a scheme that samples a missing column
    /// resolves every value to the missing colour rather than reading garbage.
    pub(crate) fn new(representation: &Representation, column: [u32; 2]) -> Self {
        let mut palette = [[0.0; 4]; PALETTE_SLOTS];
        for (class, color) in SECONDARY_STRUCTURE_COLORS.into_iter().enumerate() {
            palette[SECONDARY_BASE + class] = lanes(color);
        }
        for (index, color) in CATEGORICAL_COLORS.into_iter().enumerate() {
            palette[CATEGORICAL_BASE + index] = lanes(color);
        }
        let (selector, packed) = match representation.color {
            ColorScheme::ByChain => (SCHEME_CHAIN, 0),
            ColorScheme::ByResidue => (SCHEME_RESIDUE, 0),
            ColorScheme::BySecondaryStructure => (SCHEME_SECONDARY, 0),
            ColorScheme::ByProperty { ramp, missing, .. } => {
                for (stop, color) in ramp.colors().into_iter().enumerate() {
                    palette[RAMP_BASE + stop] = lanes(color);
                }
                let values = ramp.values();
                // The missing colour rides in the ramp row's free lane, the way
                // every other colour in this block reaches the shader.
                palette[RAMP_ROW] = [
                    values[0],
                    values[1],
                    values[2],
                    f32::from_bits(pack(missing)),
                ];
                (SCHEME_PROPERTY, 0)
            }
            ColorScheme::Uniform(color) => (SCHEME_UNIFORM, pack(color)),
            _ => (SCHEME_ELEMENT, 0),
        };
        let (appearance, softness) = match representation.appearance {
            Some(mapping) => {
                let (domain, opacity, softness_pixels, missing) = mapping.description_values();
                (
                    [domain[0], domain[1], opacity[0], opacity[1]],
                    [
                        softness_pixels[0],
                        softness_pixels[1],
                        missing[0],
                        missing[1],
                    ],
                )
            }
            None => ([0.0; 4], [0.0; 4]),
        };
        Self {
            palette,
            selector: [selector, packed, column[0], column[1]],
            appearance,
            softness,
        }
    }

    /// The palette lanes, for tests that pin the shader's arrangement.
    #[cfg(test)]
    pub(crate) fn palette_probe(&self) -> [[f32; 4]; PALETTE_SLOTS] {
        self.palette
    }

    /// Writes this block into its buffer.
    pub(super) fn write<D: Device>(self, queue: &D::Queue, buffer: &D::Buffer) {
        queue.write_buffer(buffer, 0, bytemuck::bytes_of(&self));
    }
}

/// One packed colour as four normalized lanes.
fn lanes(color: Rgba8) -> [f32; 4] {
    [
        f32::from(color.r) / 255.0,
        f32::from(color.g) / 255.0,
        f32::from(color.b) / 255.0,
        f32::from(color.a) / 255.0,
    ]
}

/// One packed colour as a single word, the form the shader unpacks.
const fn pack(color: Rgba8) -> u32 {
    u32::from_le_bytes([color.r, color.g, color.b, color.a])
}
