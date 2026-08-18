// Impostor bounds, ray unprojection and model-local clipping.
//
// The projected bound is computed once per vertex and the ray is unprojected
// on the four corners, so the fragment stage interpolates a ray instead of
// inverting a matrix per pixel. Clip planes are evaluated in model space,
// where they are static, rather than transformed per fragment.

/// Computes the conservative projected molecular bounds.
fn surface_bounds() -> SurfaceBounds {
    let root = bvh_nodes[0];

    let padding =
        vec3f(
            representation.surface.x +
            abs(representation.surface.y)
        );

    let lower =
        root.min_left.xyz - padding;

    let upper =
        root.max_radius.xyz + padding;

    var low = vec2f(1.0);
    var high = vec2f(-1.0);

    for (var corner = 0u; corner < 8u; corner++) {
        let selector =
            vec3f(
                f32(corner & 1u),
                f32((corner >> 1u) & 1u),
                f32((corner >> 2u) & 1u),
            );

        let local =
            mix(lower, upper, selector);

        let clip =
            frame.view_proj *
            vec4f(
                transform_point(
                    model.model_to_world,
                    local,
                ),
                1.0,
            );

        // Preserve the original conservative behavior: any corner behind the
        // eye promotes the impostor to fullscreen.
        if clip.w <= 0.0 {
            return SurfaceBounds(
                vec2f(-1.0),
                vec2f(1.0),
            );
        }

        let ndc =
            clip.xy * (1.0 / clip.w);

        low = min(low, ndc);
        high = max(high, ndc);
    }

    return SurfaceBounds(
        clamp(low, vec2f(-1.0), vec2f(1.0)),
        clamp(high, vec2f(-1.0), vec2f(1.0)),
    );
}

/// Unprojects one NDC point into a model-local near/far ray span.
///
/// This runs four times per draw instead of once per covered fragment.
fn surface_ray_span(ndc: vec2f) -> mat2x3f {
    let near_view =
        homogeneous_point(
            frame.inv_proj *
            vec4f(ndc, 1.0, 1.0)
        );

    let far_view =
        homogeneous_point(
            frame.inv_proj *
            vec4f(ndc, 0.0, 1.0)
        );

    let near_world =
        transform_point(
            frame.inv_view,
            near_view,
        );

    let far_world =
        transform_point(
            frame.inv_view,
            far_view,
        );

    let origin =
        transform_point(
            model.world_to_model,
            near_world,
        );

    let far_local =
        transform_point(
            model.world_to_model,
            far_world,
        );

    return mat2x3f(
        origin,
        far_local - origin,
    );
}

/// Normalizes the interpolated ray once per covered fragment.
fn surface_ray(in: SurfaceVsOut) -> SurfaceRay {
    let direction =
        normalize(in.ray_vector);

    return SurfaceRay(
        in.ray_origin,
        direction,
        1.0 / direction,
    );
}

fn ray_box(
    origin: vec3f,
    inverse_direction: vec3f,
    lower: vec3f,
    upper: vec3f,
) -> vec2f {
    let a =
        (lower - origin) *
        inverse_direction;

    let b =
        (upper - origin) *
        inverse_direction;

    let near =
        min(a, b);

    let far =
        max(a, b);

    return vec2f(
        max(
            max(near.x, near.y),
            near.z,
        ),
        min(
            min(far.x, far.y),
            far.z,
        ),
    );
}

fn clipped_local_interval(
    ray: SurfaceRay,
    interval: vec2f,
) -> RepresentationClipInterval {
    return representation_clip_interval(
        transform_point(
            model.model_to_world,
            ray.origin,
        ),
        transform_direction(
            model.model_to_world,
            ray.direction,
        ),
        interval,
    );
}

/// Converts a world-space clipping normal into model-local space.
fn local_clip_normal(plane: u32) -> vec3f {
    let world_normal =
        -representation.clip_planes[plane].xyz;

    return normalize(
        (
            transpose(model.model_to_world) *
            vec4f(world_normal, 0.0)
        ).xyz
    );
}
