// Screen-space motion vectors in UV units.
//
// Clip helpers compute only X/Y/W. Callers that already have projected XYW
// can bypass both matrix transforms through screen_motion_from_clip().

const MOTION_W_EPSILON: f32 = 1.0e-7;

/// Projects only clip-space X, Y and W.
fn clip_xyw(
    transform: mat4x4f,
    position: vec3f,
) -> vec3f {
    return transform[0].xyw * position.x
        + transform[1].xyw * position.y
        + transform[2].xyw * position.z
        + transform[3].xyw;
}

/// Returns previous-minus-current motion in UV units.
fn screen_motion_from_clip(
    current: vec3f,
    previous: vec3f,
) -> vec2f {
    if min(current.z, previous.z) <= MOTION_W_EPSILON {
        return vec2f(0.0);
    }

    let inverse_w =
        vec2f(1.0) /
        vec2f(current.z, previous.z);

    let delta =
        previous.xy * inverse_w.y -
        current.xy * inverse_w.x;

    return delta * vec2f(0.5, -0.5);
}

/// Returns the history offset from current to previous screen position.
fn screen_motion(
    current_world: vec3f,
    previous_world: vec3f,
) -> vec2f {
    return screen_motion_from_clip(
        clip_xyw(
            frame.view_proj,
            current_world,
        ),
        clip_xyw(
            frame.previous_view_proj,
            previous_world,
        ),
    );
}
