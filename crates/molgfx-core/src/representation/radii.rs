//! Display constants per element: van der Waals radii and CPK colors.
//!
//! These are presentation defaults, not chemistry: the radius set is the
//! community-standard one (Bondi 1964 with common extensions) and the colors
//! follow the CPK convention scientists already read fluently. Both are
//! indexed by atomic number in `O(1)`.

use molgfx_math::Rgba8;

/// The radius drawn for an element no table covers, in Ångström.
const DEFAULT_RADIUS: f32 = 1.7;

/// Van der Waals radius in Ångström for an atomic number.
#[must_use]
pub fn vdw_radius(atomic_number: u8) -> f32 {
    match atomic_number {
        1 => 1.20,       // H
        2 => 1.40,       // He
        3 => 1.82,       // Li
        6 => 1.70,       // C
        7 => 1.55,       // N
        8 => 1.52,       // O
        9 => 1.47,       // F
        11 => 2.27,      // Na
        12 => 1.73,      // Mg
        14 => 2.10,      // Si
        15 | 16 => 1.80, // P, S
        17 => 1.75,      // Cl
        19 => 2.75,      // K
        20 => 2.31,      // Ca
        25 => 2.05,      // Mn
        26 => 2.04,      // Fe
        27 => 2.00,      // Co
        28 => 1.97,      // Ni
        29 => 1.96,      // Cu
        30 => 2.01,      // Zn
        34 => 1.90,      // Se
        35 => 1.85,      // Br
        53 => 1.98,      // I
        _ => DEFAULT_RADIUS,
    }
}

/// CPK display color for an atomic number.
#[must_use]
pub fn cpk_color(atomic_number: u8) -> Rgba8 {
    match atomic_number {
        1 => Rgba8::opaque(0xE8, 0xE8, 0xE8),       // H, near-white
        6 => Rgba8::opaque(0x55, 0x55, 0x55),       // C, dark grey
        7 => Rgba8::opaque(0x32, 0x64, 0xC8),       // N, blue
        8 => Rgba8::opaque(0xD8, 0x28, 0x28),       // O, red
        9 => Rgba8::opaque(0x7F, 0xC8, 0x3F),       // F, green
        15 => Rgba8::opaque(0xFF, 0x8C, 0x1A),      // P, orange
        16 => Rgba8::opaque(0xE6, 0xC8, 0x28),      // S, yellow
        17 => Rgba8::opaque(0x1F, 0xC0, 0x1F),      // Cl, green
        26 => Rgba8::opaque(0xC8, 0x78, 0x28),      // Fe, ochre
        30 => Rgba8::opaque(0x7D, 0x80, 0xB0),      // Zn, slate
        34 => Rgba8::opaque(0xFF, 0xA1, 0x00),      // Se, deep orange
        35 => Rgba8::opaque(0xA6, 0x2A, 0x2A),      // Br, dark red
        53 => Rgba8::opaque(0x94, 0x0F, 0x94),      // I, violet
        11 | 19 => Rgba8::opaque(0xAB, 0x5C, 0xF2), // Na, K, purple
        12 | 20 => Rgba8::opaque(0x3C, 0xB4, 0x3C), // Mg, Ca, green
        _ => Rgba8::opaque(0xE0, 0x8A, 0xC8),       // unknown, CPK pink
    }
}
