// Interaction-state vocabulary shared by the fragment stage and the lighting
// pass.
//
// A fragment folds its strongest marker into the gbuffer material payload, so
// the lighting pass can draw a marker edge from the attachments it already
// reads instead of a dedicated mask target. The file declares no bindings, so
// both stages can include it.

const INTERACTION_SELECTED: u32 = 1u;
const INTERACTION_HOVERED: u32 = 2u;
const INTERACTION_FOCUSED: u32 = 4u;
const INTERACTION_MUTED: u32 = 8u;
const INTERACTION_HIDDEN: u32 = 16u;

const MARKER_NONE: u32 = 0u;
const MARKER_HOVERED: u32 = 1u;
const MARKER_FOCUSED: u32 = 2u;
const MARKER_SELECTED: u32 = 3u;

// Material payloads stay below this bound (shading model 0..7 plus the
// emission flag of 8), so the marker occupies the quotient above it.
const MARKER_PAYLOAD_STEP: f32 = 16.0;

const SELECTED_TINT: vec3f = vec3f(1.0, 0.58, 0.08);
const HOVERED_TINT: vec3f = vec3f(1.0, 0.25, 0.75);
const FOCUSED_TINT: vec3f = vec3f(0.25, 0.78, 1.0);

/// The strongest marker a state word carries: selected over focused over hovered.
fn marker_of_state(state: u32) -> u32 {
    if (state & INTERACTION_SELECTED) != 0u {
        return MARKER_SELECTED;
    }
    if (state & INTERACTION_FOCUSED) != 0u {
        return MARKER_FOCUSED;
    }
    if (state & INTERACTION_HOVERED) != 0u {
        return MARKER_HOVERED;
    }
    return MARKER_NONE;
}

fn marker_tint(marker: u32) -> vec3f {
    if marker == MARKER_SELECTED {
        return SELECTED_TINT;
    }
    if marker == MARKER_FOCUSED {
        return FOCUSED_TINT;
    }
    return HOVERED_TINT;
}

fn marker_encode_payload(payload: f32, marker: u32) -> f32 {
    return payload + f32(marker) * MARKER_PAYLOAD_STEP;
}

fn marker_from_payload(encoded: f32) -> u32 {
    return u32(max(floor(encoded / MARKER_PAYLOAD_STEP), 0.0));
}

fn payload_without_marker(encoded: f32) -> f32 {
    return encoded - f32(marker_from_payload(encoded)) * MARKER_PAYLOAD_STEP;
}
