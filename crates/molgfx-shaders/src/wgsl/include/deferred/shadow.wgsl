// Percentage-closer shadow filtering against the analytic shadow map.
//
// Interior and boundary taps are separate paths: a pixel well inside the map
// skips the clip tests a boundary pixel needs. Cost is O(taps) per lit
// fragment, with the tap count fixed so the pass cannot exceed its budget.

fn shadow_compare(
    coordinate: vec2i,
    biased_depth: f32,
) -> f32 {
    let stored_depth =
        textureLoad(
            shadow_texture,
            coordinate,
            0,
        ).x;

    return select(
        0.0,
        1.0,
        biased_depth >= stored_depth,
    );
}

fn shadow_pcf_interior(
    center: vec2i,
    biased_depth: f32,
) -> f32 {
    let visible =
        shadow_compare(
            center + vec2i(-1, -1),
            biased_depth,
        )
        + shadow_compare(
            center + vec2i(1, -1),
            biased_depth,
        )
        + shadow_compare(
            center + vec2i(-1, 1),
            biased_depth,
        )
        + shadow_compare(
            center + vec2i(1, 1),
            biased_depth,
        );

    return visible *
        SHADOW_INV_TAP_COUNT;
}

fn shadow_pcf_boundary(
    center: vec2i,
    dimensions: vec2i,
    biased_depth: f32,
) -> f32 {
    var visible = 0.0;

    for (
        var index = 0u;
        index < SHADOW_TAP_COUNT;
        index++
    ) {
        let coordinate =
            center +
            SHADOW_OFFSETS[index];

        if coordinate.x < 0
            || coordinate.y < 0
            || coordinate.x >= dimensions.x
            || coordinate.y >= dimensions.y {
            visible += 1.0;
            continue;
        }

        visible +=
            shadow_compare(
                coordinate,
                biased_depth,
            );
    }

    return visible *
        SHADOW_INV_TAP_COUNT;
}

fn shadow_clip_visibility(
    clip: vec4f,
) -> f32 {
    if clip.w <= 0.0 {
        return 1.0;
    }

    let inv_w =
        1.0 /
        clip.w;

    let ndc =
        clip.xy *
        inv_w;

    if any(
        abs(ndc) >
        vec2f(1.0)
    ) {
        return 1.0;
    }

    let dimensions =
        vec2i(
            textureDimensions(
                shadow_texture,
            ),
        );

    let uv =
        ndc * 0.5 +
        0.5;

    // uv is non-negative here, so integer conversion is equivalent to floor.
    let center =
        vec2i(
            uv *
            vec2f(dimensions),
        );

    let biased_depth =
        clip.z *
        inv_w +
        SHADOW_BIAS;

    // Almost all samples are interior. Pay the per-tap boundary checks only
    // for the thin border of the shadow map.
    if center.x > 0
        && center.y > 0
        && center.x < dimensions.x - 1
        && center.y < dimensions.y - 1 {
        return shadow_pcf_interior(
            center,
            biased_depth,
        );
    }

    return shadow_pcf_boundary(
        center,
        dimensions,
        biased_depth,
    );
}

fn shadow_visibility(
    world_position: vec3f,
    normal: vec3f,
) -> f32 {
    let clip =
        frame.shadow_view_proj *
        vec4f(
            world_position +
                normal *
                SHADOW_NORMAL_OFFSET,
            1.0,
        );

    return shadow_clip_visibility(
        clip,
    );
}

fn shadow_visibility_view(
    view_position_value: vec3f,
    view_normal: vec3f,
) -> f32 {
    // Offset while both values are in view space, then transform the point.
    // This avoids mixing a world-space position with a view-space normal.
    let offset_position =
        view_position_value +
        view_normal *
        SHADOW_NORMAL_OFFSET;

    let world_position =
        frame.inv_view[0].xyz *
            offset_position.x
        + frame.inv_view[1].xyz *
            offset_position.y
        + frame.inv_view[2].xyz *
            offset_position.z
        + frame.inv_view[3].xyz;

    let clip =
        frame.shadow_view_proj *
        vec4f(
            world_position,
            1.0,
        );

    return shadow_clip_visibility(
        clip,
    );
}

fn direct_visibility(
    position: vec3f,
    normal: vec3f,
    ao_visibility: f32,
) -> f32 {
    if ao_visibility == 0.0 {
        return 0.0;
    }

    return ao_visibility *
        shadow_visibility_view(
            position,
            normal,
        );
}
