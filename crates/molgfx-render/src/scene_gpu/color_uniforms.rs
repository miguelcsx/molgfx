//! The colour block the GPU schemes resolve against.
//!
//! Colour used to be resolved on the CPU and baked into each instance record,
//! so changing a scheme repacked and re-uploaded every atom. The record now
//! carries only the element colour; this block carries which scheme applies and
//! where its inputs live, so changing a scheme is one fixed-size uniform write.
//!
//! Every scheme is a *rule*: a tag, an optional packed colour, and the arena
//! column it reads. The representation's base scheme is one rule; each entry of
//! a selection-scoped overlay is another, and both are resolved by the same
//! shader function. Categorical rules index the shared palette bank, which
//! holds every built-in palette and is therefore identical in every block.

use super::ramp_lut::RampLut;
use molgfx_core::{
    CategoryPalette, ColorColumns, ColorOverlay, ColorScheme, MAX_COLOR_OVERLAY_CLASSES,
    Representation,
};
use molgfx_gpu::{Device, Queue as _};
use molgfx_math::Rgba8;

/// The rule tags the shader switches on.
///
/// They are shader constants; naming them here is what keeps a rule's tag a
/// single word rather than a set of flags that could disagree.
const RULE_ELEMENT: u32 = 0;
const RULE_UNIFORM: u32 = 1;
const RULE_PROPERTY: u32 = 2;
const RULE_CATEGORY: u32 = 3;

/// Colours the palette bank holds: enough for every built-in palette at once.
pub(crate) const BANK_COLORS: usize = 128;
/// Packed colours per bank element.
const BANK_LANES: usize = 4;

/// The arena column an atom-row scalar is read from: offset and stride words.
///
/// A zero offset means no column was planned; the shader then treats every
/// value as missing rather than reading an unrelated column.
pub(crate) type Column = [u32; 2];

/// The arena columns one representation's colour reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ResolvedColumns {
    /// The base scheme's column.
    pub(crate) base: Column,
    /// The overlay class column.
    pub(crate) overlay: Column,
    /// The column each overlay scheme reads, class one first.
    pub(crate) schemes: [Column; MAX_COLOR_OVERLAY_CLASSES],
    /// The appearance mapping's column.
    pub(crate) appearance: Column,
}

impl ResolvedColumns {
    /// No column planned anywhere.
    pub(crate) const NONE: Self = Self {
        base: [0, 1],
        overlay: [0, 1],
        schemes: [[0, 1]; MAX_COLOR_OVERLAY_CLASSES],
        appearance: [0, 1],
    };

    /// Resolves each handle `columns` names through `locate`.
    pub(crate) fn resolve(
        columns: &ColorColumns,
        locate: impl Fn(molgfx_core::AtomPropertyHandle) -> Column,
    ) -> Self {
        let find = |handle: Option<molgfx_core::AtomPropertyHandle>| match handle {
            Some(handle) => locate(handle),
            None => [0, 1],
        };
        let mut resolved = Self::NONE;
        resolved.base = find(columns.base);
        resolved.overlay = find(columns.overlay_classes);
        resolved.appearance = find(columns.appearance);
        for (slot, handle) in resolved.schemes.iter_mut().zip(columns.overlay_schemes) {
            *slot = find(handle);
        }
        resolved
    }
}

/// One colour rule, byte-identical to the shader's `ColorRule`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
struct Rule {
    /// Rule tag, packed uniform colour, column offset, column stride.
    header: [u32; 4],
    /// Palette bank base, palette length, and two reserved words.
    palette: [u32; 4],
}

/// The colour block, byte-identical to the shader's `ColorUniforms`.
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct ColorUniforms {
    /// The representation's own scheme.
    base: Rule,
    /// Appearance domain in `xy` and its opacity response in `zw`.
    ///
    /// A domain that does not increase means no appearance mapping is active,
    /// which is what its own constructor guarantees, so no separate flag is
    /// needed.
    appearance: [f32; 4],
    /// The softness response in `xy` and the missing-value response in `zw`.
    softness: [f32; 4],
    /// The appearance column: arena offset, stride, and two reserved words.
    appearance_column: [u32; 4],
    /// The selection-scoped overlay: the class column's arena offset (zero
    /// when none applies), its stride in words, and the number of classes.
    overlay: [u32; 4],
    /// One rule per overlay class, class one first.
    overlay_rules: [Rule; MAX_COLOR_OVERLAY_CLASSES],
    /// Every built-in palette, packed four colours to an element.
    bank: [[u32; BANK_LANES]; BANK_COLORS / BANK_LANES],
    /// The property ramp, baked to a lookup table; inert for other schemes.
    ramp: RampLut,
}

impl ColorUniforms {
    /// Builds the block for one representation.
    pub(crate) fn new(representation: &Representation, columns: &ResolvedColumns) -> Self {
        let mut ramp = RampLut::disabled();
        let base = match representation.color {
            ColorScheme::ByProperty {
                ramp: property_ramp,
                missing,
                ..
            } => {
                ramp = RampLut::new(&property_ramp, missing);
                rule(RULE_PROPERTY, 0, columns.base, None)
            }
            scheme => scheme_rule(scheme, columns.base),
        };
        let (overlay, overlay_rules) = overlay_block(representation, columns);
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
            base,
            appearance,
            softness,
            appearance_column: [columns.appearance[0], columns.appearance[1], 0, 0],
            overlay,
            overlay_rules,
            bank: palette_bank(),
            ramp,
        }
    }

    /// The overlay header and rule headers, for tests that pin the layout.
    #[cfg(test)]
    pub(crate) fn overlay_probe(&self) -> ([u32; 4], Vec<[u32; 4]>) {
        (
            self.overlay,
            self.overlay_rules.iter().map(|rule| rule.header).collect(),
        )
    }

    /// One bank colour as the shader would read it, for tests.
    #[cfg(test)]
    pub(crate) const fn bank_probe(&self, index: usize) -> u32 {
        self.bank[index / BANK_LANES][index % BANK_LANES]
    }

    /// Writes this block into its buffer.
    pub(super) fn write<D: Device>(self, queue: &D::Queue, buffer: &D::Buffer) {
        queue.write_buffer(buffer, 0, bytemuck::bytes_of(&self));
    }
}

/// The bank offset at which `palette` starts.
///
/// Palettes sit back to back in [`CategoryPalette::ALL`] order, so an offset is
/// a sum of lengths and identical on every block.
pub(crate) fn bank_base(palette: CategoryPalette) -> usize {
    CategoryPalette::ALL
        .into_iter()
        .take_while(|other| *other != palette)
        .map(CategoryPalette::len)
        .sum()
}

/// Every built-in palette, back to back.
fn palette_bank() -> [[u32; BANK_LANES]; BANK_COLORS / BANK_LANES] {
    let mut bank = [[0_u32; BANK_LANES]; BANK_COLORS / BANK_LANES];
    let mut index = 0;
    for palette in CategoryPalette::ALL {
        for color in palette.colors() {
            bank[index / BANK_LANES][index % BANK_LANES] = pack(*color);
            index += 1;
        }
    }
    bank
}

/// The rule one scheme resolves to.
///
/// A continuous property ramp belongs to the block's single ramp table and is
/// handled by the caller; it degrades to the element colour anywhere else.
fn scheme_rule(scheme: ColorScheme, column: Column) -> Rule {
    match scheme {
        ColorScheme::Uniform(color) => rule(RULE_UNIFORM, pack(color), [0, 1], None),
        ColorScheme::ByCategory { palette, .. } => rule(RULE_CATEGORY, 0, column, Some(palette)),
        _ => rule(RULE_ELEMENT, 0, [0, 1], None),
    }
}

fn rule(tag: u32, packed: u32, column: Column, palette: Option<CategoryPalette>) -> Rule {
    let palette_words = match palette {
        Some(palette) => [word(bank_base(palette)), word(palette.len()), 0, 0],
        None => [0; 4],
    };
    Rule {
        header: [tag, packed, column[0], column[1]],
        palette: palette_words,
    }
}

/// The overlay header and rule table for one representation.
///
/// An overlay whose class column was not planned into the arena is disabled
/// rather than pointed at offset zero, so the shader never reads an unrelated
/// column.
fn overlay_block(
    representation: &Representation,
    columns: &ResolvedColumns,
) -> ([u32; 4], [Rule; MAX_COLOR_OVERLAY_CLASSES]) {
    let mut rules = [Rule::default(); MAX_COLOR_OVERLAY_CLASSES];
    let Some(overlay) = representation.color_overlay else {
        return ([0, 1, 0, 0], rules);
    };
    if columns.overlay[0] == 0 {
        return ([0, 1, 0, 0], rules);
    }
    fill_overlay_rules(&overlay, columns, &mut rules);
    let count = word(overlay.schemes().len());
    ([columns.overlay[0], columns.overlay[1], count, 0], rules)
}

fn fill_overlay_rules(
    overlay: &ColorOverlay,
    columns: &ResolvedColumns,
    rules: &mut [Rule; MAX_COLOR_OVERLAY_CLASSES],
) {
    for ((slot, scheme), column) in rules.iter_mut().zip(overlay.schemes()).zip(columns.schemes) {
        *slot = scheme_rule(*scheme, column);
    }
}

/// A small count as a uniform word; the counts here are bounded far below `u32`.
fn word(count: usize) -> u32 {
    let Ok(word) = u32::try_from(count) else {
        return u32::MAX;
    };
    word
}

/// One colour as a single word, the form the shader unpacks.
const fn pack(color: Rgba8) -> u32 {
    u32::from_le_bytes([color.r, color.g, color.b, color.a])
}
