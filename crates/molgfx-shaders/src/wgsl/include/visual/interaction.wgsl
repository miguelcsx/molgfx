// Shared interaction-state presentation for every visual representation.
//
// The state column is addressed through the visual arena, so atom, bond,
// point and ribbon shaders all read one authoritative GPU policy.

//!include "include/visual/marker.wgsl"

fn interaction_state(source: u32) -> u32 {
    return visual_properties[visual_config.arena_offsets.z + source];
}

fn interaction_marker(source: u32) -> u32 {
    return marker_of_state(interaction_state(source));
}

fn interaction_hidden(source: u32) -> bool {
    return (interaction_state(source) & INTERACTION_HIDDEN) != 0u;
}

/// Applies selection, hover, focus and mute without changing immutable records.
fn interaction_color(color: vec4f, source: u32) -> vec4f {
    let state = interaction_state(source);
    var result = color;
    if (state & INTERACTION_SELECTED) != 0u {
        result = vec4f(mix(result.rgb, SELECTED_TINT, 0.72), result.a);
    }
    if (state & INTERACTION_HOVERED) != 0u {
        result = vec4f(mix(result.rgb, vec3f(1.0), 0.38), result.a);
    }
    if (state & INTERACTION_FOCUSED) != 0u {
        result = vec4f(mix(result.rgb, FOCUSED_TINT, 0.34), result.a);
    }
    if (state & INTERACTION_MUTED) != 0u {
        result = vec4f(result.rgb * 0.45, result.a * 0.32);
    }
    return result;
}
