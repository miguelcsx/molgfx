const LUMA_WEIGHTS: vec3f = vec3f(0.2126, 0.7152, 0.0722);
const CHROMA_FULL_WEIGHT_SQ: f32 = 0.10 * 0.10;
const CHROMA_REJECT_SQ: f32 = 0.28 * 0.28;

// Peak ghost strength for a disoccluded pixel at full trail persistence.
const DISOCCLUSION_TRAIL_STRENGTH: f32 = 0.82;

struct TemporalCross {
    fallback: vec3f,
    minimum: vec3f,
    maximum: vec3f,
}

struct ColorBounds {
    minimum: vec3f,
    maximum: vec3f,
}

fn luma(color: vec3f) -> f32 {
    return dot(color, LUMA_WEIGHTS);
}

fn load_current(pixel: vec2i, dimensions: vec2i) -> vec3f {
    return textureLoad(
        current_hdr,
        clamp(pixel, vec2i(0), dimensions - 1),
        0,
    ).rgb;
}

/// Loads the four cardinal neighbors once for both fallback and clamp bounds.
fn temporal_cross(pixel: vec2i, dimensions: vec2i, center: vec3f) -> TemporalCross {
    let north = load_current(pixel + vec2i(0, -1), dimensions);
    let south = load_current(pixel + vec2i(0, 1), dimensions);
    let west = load_current(pixel + vec2i(-1, 0), dimensions);
    let east = load_current(pixel + vec2i(1, 0), dimensions);
    let horizontal = abs(luma(west) - luma(east));
    let vertical = abs(luma(north) - luma(south));
    let edge = max(horizontal, vertical);
    var fallback = center;
    if edge >= 0.025 {
        let pair = select(
            (north + south) * 0.5,
            (west + east) * 0.5,
            vertical > horizontal,
        );
        fallback = mix(center, pair, min(edge * 0.18, 0.16));
    }
    return TemporalCross(
        fallback,
        min(center, min(min(north, south), min(west, east))),
        max(center, max(max(north, south), max(west, east))),
    );
}

/// Adds only the four missing diagonal texels to the 3x3 clamp bounds.
fn temporal_neighborhood_bounds(
    pixel: vec2i,
    dimensions: vec2i,
    cross: TemporalCross,
) -> ColorBounds {
    let northwest = load_current(pixel + vec2i(-1, -1), dimensions);
    let northeast = load_current(pixel + vec2i(1, -1), dimensions);
    let southwest = load_current(pixel + vec2i(-1, 1), dimensions);
    let southeast = load_current(pixel + vec2i(1, 1), dimensions);
    return ColorBounds(
        min(
            cross.minimum,
            min(min(northwest, northeast), min(southwest, southeast)),
        ),
        max(
            cross.maximum,
            max(max(northwest, northeast), max(southwest, southeast)),
        ),
    );
}
