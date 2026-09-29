// Scalar colour ramps as a fixed lookup table.
//
// A ramp of up to sixteen stops is baked on the host into 256 packed
// colours across its domain, so every fragment resolves any ramp with two
// table reads and one mix, whatever the stop count. The table is exact at the
// stops to within one 1/255 step of the domain, which is finer than the eight
// bits each colour channel carries.
//
// The struct is declared here once and embedded by every uniform block that
// carries a ramp, so atom property colours and surface scalar overlays cannot
// drift apart.

const RAMP_LUT_SIZE: u32 = 256u;
const RAMP_LUT_LAST: f32 = 255.0;

struct RampLut {
    /// x = first stop value, y = 255 / (last stop − first stop),
    /// z = the missing colour as packed rgba8 bits, w = reserved.
    domain: vec4f,
    /// 256 packed rgba8 colours, four per element.
    colors: array<vec4u, 64>,
}

/// The two table entries a value falls between and the weight of the upper.
struct RampTap {
    low: u32,
    high: u32,
    fraction: f32,
}

/// Locates `value` in a table whose domain starts at `first` and spans
/// `scale` table steps per unit. A value outside the domain clamps.
fn ramp_tap(value: f32, first: f32, scale: f32) -> RampTap {
    let position = clamp((value - first) * scale, 0.0, RAMP_LUT_LAST);
    let low = u32(floor(position));
    return RampTap(low, min(low + 1u, RAMP_LUT_SIZE - 1u), position - f32(low));
}

/// Blends two packed table entries.
fn ramp_mix(low: u32, high: u32, fraction: f32) -> vec4f {
    return mix(unpack4x8unorm(low), unpack4x8unorm(high), fraction);
}
