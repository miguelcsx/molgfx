// Marker edges: a one-pixel outline around selected, focused and hovered
// geometry, drawn from the marker each fragment folded into its material
// payload.
//
// The marker mask is 0 where a fragment is marked and 1 elsewhere. An edge
// exists wherever the mask differs across a pixel's axis neighbours, and its
// strength is the length of the two axis differences, so straight borders and
// corners weight naturally. Cost is four gbuffer loads per shaded pixel and no
// extra attachment or pass; a pixel whose own and neighbouring markers are all
// clear returns immediately after those loads.

const MARKER_NEIGHBOURS: array<vec2i, 4> = array<vec2i, 4>(
    vec2i(1, 0),
    vec2i(-1, 0),
    vec2i(0, 1),
    vec2i(0, -1),
);

// Brightens the inner side of an edge so the outline reads as a rim.
const MARKER_INNER_GAIN: f32 = 1.5;
const MARKER_EDGE_ALPHA: f32 = 2.0;

fn marker_at(pixel: vec2i, dimensions: vec2i) -> u32 {
    let clamped = clamp(pixel, vec2i(0), dimensions - 1);
    return marker_from_payload(textureLoad(albedo_texture, clamped, 0).a);
}

fn apply_marker_edge(
    color: vec3f,
    pixel: vec2i,
    dimensions: vec2i,
    centre: u32,
) -> vec3f {
    var strongest = centre;
    var mask = array<f32, 4>(1.0, 1.0, 1.0, 1.0);
    for (var i = 0u; i < 4u; i += 1u) {
        let neighbour = marker_at(pixel + MARKER_NEIGHBOURS[i], dimensions);
        mask[i] = select(1.0, 0.0, neighbour != MARKER_NONE);
        strongest = max(strongest, neighbour);
    }
    if strongest == MARKER_NONE {
        return color;
    }

    // The inner and outer pixel of a border both see differing neighbours, so
    // the outline is two pixels wide and the inner side is brightened below.
    let across = vec2f(mask[0] - mask[1], mask[2] - mask[3]) * 0.5;
    let strength = length(across);
    if strength <= 0.0 {
        return color;
    }

    var tint = marker_tint(strongest);
    if centre != MARKER_NONE {
        tint = min(tint * MARKER_INNER_GAIN, vec3f(1.0));
    }
    return mix(color, tint, min(strength * MARKER_EDGE_ALPHA, 1.0));
}
