//! Bit layout of the packed per-atom semantic word.
//!
//! The word is a `u32` whose low 24 bits carry caller site and band tags, whose
//! top byte carries quantized edge softness, and whose middle nine bits carry
//! the three palette indices the GPU colour schemes resolve from. Keeping the
//! layout in one place is what lets the packer and the shaders agree without
//! either restating it.

/// Bit layout of [`AtomGpu::semantic`].
///
/// The low 24 bits carry caller site and band tags; the top byte carries
/// quantized edge softness, which the point and sphere shaders decode. The
/// three palette indices sit between them, in bits nothing else uses, so the
/// record stays exactly 20 bytes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SemanticTag;

impl SemanticTag {
    /// Bits 10-12: the atom's chain, reduced modulo the palette length.
    pub const CHAIN_SHIFT: u32 = 10;
    /// Bits 13-15: the atom's residue row, reduced modulo the palette length.
    ///
    /// The CPU path already reduced the residue row the same way, so the GPU
    /// reproduces it exactly rather than approximating it.
    pub const RESIDUE_SHIFT: u32 = 13;
    /// Bits 16-18: the residue's secondary-structure class.
    pub const SECONDARY_SHIFT: u32 = 16;
    /// Bits 0-23 are the caller's own tags.
    pub const TAG_MASK: u32 = 0x0000_ffff;
    /// Bits 10-18 hold the three palette indices.
    pub const COLOR_MASK: u32 = 0x0007_fc00;
    /// Bits 24-31 hold quantized edge softness.
    pub const SOFTNESS_SHIFT: u32 = 24;
    /// The three indices occupy this many bits each.
    pub const FIELD_BITS: u32 = 3;
    /// The largest index one field can hold.
    pub const FIELD_MAX: u32 = 7;
}
