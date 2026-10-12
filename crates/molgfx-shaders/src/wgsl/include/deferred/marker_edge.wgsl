// Marker edges: an outline and a wash of colour on selected, focused and
// hovered geometry, drawn from the marker each fragment folded into its
// material payload.
//
// The marker mask is 0 where a fragment is marked and 1 elsewhere. An edge
// exists wherever the mask differs across a pixel's axis neighbours, and its
// strength is the length of the two axis differences, so straight borders and
// corners weight naturally. A pixel with no marker within reach pays four
// gbuffer loads and returns; a marked pixel pays four more, for the second
// ring that makes the outline wide enough to see. No extra attachment or pass.

const MARKER_NEIGHBOURS: array<vec2i, 4> = array<vec2i, 4>(
    vec2i(1, 0),
    vec2i(-1, 0),
    vec2i(0, 1),
    vec2i(0, -1),
);

// Brightens the inner side of an edge so the outline reads as a rim.
const MARKER_INNER_GAIN: f32 = 1.5;
const MARKER_EDGE_ALPHA: f32 = 2.0;
// How far marked geometry is washed toward its marker colour, so a selection
// reads on thin or small geometry that has no interior edge, and against a
// palette that happens to share its hue.
const MARKER_FILL_SELECTED: f32 = 0.34;
const MARKER_FILL_FOCUSED: f32 = 0.14;
const MARKER_FILL_HOVERED: f32 = 0.22;
// Pixels of render height per pixel of outline: a frame twice as tall draws an
// outline twice as wide, so it keeps the same size on a high-density screen.
const MARKER_PIXELS_PER_STEP: f32 = 700.0;

fn marker_at(pixel: vec2i, dimensions: vec2i) -> u32 {
    let clamped = clamp(pixel, vec2i(0), dimensions - 1);
    return marker_from_payload(textureLoad(albedo_texture, clamped, 0).a);
}

fn marker_fill(marker: u32) -> f32 {
    if marker == MARKER_SELECTED {
        return MARKER_FILL_SELECTED;
    }
    if marker == MARKER_FOCUSED {
        return MARKER_FILL_FOCUSED;
    }
    if marker == MARKER_HOVERED {
        return MARKER_FILL_HOVERED;
    }
    return 0.0;
}

struct MarkerRing {
    strength: f32,
    strongest: u32,
}

// The edge strength across the four axis neighbours `radius` pixels away, and
// the strongest marker among them and the centre.
fn marker_ring(pixel: vec2i, dimensions: vec2i, radius: i32, centre: u32) -> MarkerRing {
    var strongest = centre;
    var mask = array<f32, 4>(1.0, 1.0, 1.0, 1.0);
    for (var i = 0u; i < 4u; i += 1u) {
        let neighbour = marker_at(pixel + MARKER_NEIGHBOURS[i] * radius, dimensions);
        mask[i] = select(1.0, 0.0, neighbour != MARKER_NONE);
        strongest = max(strongest, neighbour);
    }
    // The marker mask is 0 where marked: an edge is where it differs across the
    // pixel, and straight borders and corners weight naturally.
    let across = vec2f(mask[0] - mask[1], mask[2] - mask[3]) * 0.5;
    return MarkerRing(length(across), strongest);
}

fn apply_marker_edge(
    color: vec3f,
    pixel: vec2i,
    dimensions: vec2i,
    centre: u32,
) -> vec3f {
    let step = max(1, i32(round(f32(dimensions.y) / MARKER_PIXELS_PER_STEP)));
    let near = marker_ring(pixel, dimensions, step, centre);
    var strongest = near.strongest;
    var strength = near.strength;
    var shaded = color;
    if centre != MARKER_NONE {
        // Inside marked geometry the outline is two rings deep, so it is wide
        // enough to see without costing the pixels far from any marker a thing.
        let far = marker_ring(pixel, dimensions, step * 2, centre);
        strength = max(strength, far.strength * 0.7);
        shaded = mix(shaded, marker_tint(centre), marker_fill(centre));
    }
    if strongest == MARKER_NONE {
        return shaded;
    }
    if strength <= 0.0 {
        return shaded;
    }

    // Two tones, so the rim reads on any colour: bright on the marked side, a
    // dark shadow just outside it. A single hue always clashes with something.
    var tint = marker_tint(strongest);
    if centre != MARKER_NONE {
        tint = min(tint * MARKER_INNER_GAIN, vec3f(1.0));
        return mix(shaded, tint, min(strength * MARKER_EDGE_ALPHA, 1.0));
    }
    return mix(shaded, vec3f(0.03), min(strength * MARKER_EDGE_ALPHA, 1.0) * 0.85);
}
