//! Explicit conversions at the instruction-payload boundary.
//!
//! A visual instruction carries four floats. Some opcodes use one of them to
//! name a slot — a property column, a parameter — rather than to carry a
//! value, so an index has to travel through a float field. Every such index is
//! bounded by a program limit well under the point where an `f32` stops
//! counting integers exactly, which is what makes the round trip lossless; the
//! named helpers keep that argument attached to the conversion instead of
//! leaving a bare cast for a reader to reason about.

/// Writes a slot index into an instruction's float payload.
///
/// Program limits cap every slot far below `f32`'s exact-integer range, so the
/// value round-trips through [`decode_slot`] unchanged.
pub(super) fn encode_slot(slot: usize) -> f32 {
    match u16::try_from(slot) {
        Ok(slot) => f32::from(slot),
        // Unreachable for a validated program; saturating keeps the encoder
        // total, and decoding the result yields an out-of-range slot that the
        // reader already handles as a miss.
        Err(_) => f32::from(u16::MAX),
    }
}

/// Reads a slot index previously written by [`encode_slot`].
///
/// A negative or non-finite payload cannot name a slot, so it decodes to an
/// index past every table — which every reader already treats as absent.
pub(super) fn decode_slot(value: f32) -> usize {
    if value.is_finite() && value >= 0.0 && value <= f32::from(u16::MAX) {
        let bits = value.to_bits();
        let exponent = (bits >> 23) & 0xff;
        if exponent < 127 {
            return 0;
        }
        let shift = exponent - 127;
        let mantissa = (bits & 0x7f_ffff) | 0x80_0000;
        let integer = if shift <= 23 {
            mantissa >> (23 - shift)
        } else {
            mantissa << (shift - 23)
        };
        match usize::try_from(integer) {
            Ok(slot) => slot,
            Err(_) => usize::MAX,
        }
    } else {
        usize::MAX
    }
}

/// Reads a small opcode discriminant from an instruction's float payload.
pub(super) fn decode_code(value: f32) -> u8 {
    let slot = decode_slot(value);
    match u8::try_from(slot) {
        Ok(code) => code,
        Err(_) => u8::MAX,
    }
}

/// Converts a stable entity row to the scalar a program reads.
///
/// Rows past twenty-four bits cannot be represented exactly, and the nearest
/// float is the honest answer: a program that compares entity rows for equality
/// stops being reliable at that scale, which is why the builder documents the
/// input as exact only while representable.
pub(super) fn entity_scalar(entity: u32) -> f32 {
    let high = match u16::try_from(entity >> 16) {
        Ok(high) => high,
        Err(_) => u16::MAX,
    };
    let low = match u16::try_from(entity & u32::from(u16::MAX)) {
        Ok(low) => low,
        Err(_) => u16::MAX,
    };
    f32::from(high).mul_add(65_536.0, f32::from(low))
}
