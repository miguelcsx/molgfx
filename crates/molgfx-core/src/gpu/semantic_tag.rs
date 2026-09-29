//! Bit layout of the packed per-atom semantic word.
//!
//! The word is a `u32` whose low sixteen bits carry caller site and band tags
//! and whose top byte carries quantized edge softness. The middle byte is
//! reserved. Colour is not in the word: schemes resolve from scene property
//! columns, so a colour scheme never limits how many chains, entities or
//! residue kinds the word can distinguish.

/// Bit layout of [`AtomGpu::semantic`](super::AtomGpu::semantic).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SemanticTag;

impl SemanticTag {
    /// Bits 0-15 are the caller's own tags.
    pub const TAG_MASK: u32 = 0x0000_ffff;
    /// Bits 24-31 hold quantized edge softness.
    pub const SOFTNESS_SHIFT: u32 = 24;
}
