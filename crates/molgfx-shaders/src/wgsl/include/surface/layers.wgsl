// Layered compositing of a translucent grid surface.
//
// The blended pass takes one fragment per pixel from a surface impostor, so a
// ray that crosses the boundary several times would otherwise show only its
// nearest crossing and the far wall and cavities of a translucent solid would
// vanish. The crossings are composited here, front to back with the over
// operator, and handed to the blended pass as one premultiplied contribution
// carrying the nearest crossing's depth.

/// The ray parameter at which this pixel's ray meets the opaque scene, or
/// infinity where nothing opaque covers the pixel.
///
/// The opaque depth is reversed-Z at the pixel, so it unprojects through the
/// same inverse chain that built the ray and lands on the ray's own line; the
/// parameter is then its distance along the normalized local direction.
fn surface_opaque_limit(
    fragment_position: vec4f,
    ray: SurfaceRay,
) -> f32 {
    let depth = textureLoad(
        oit_depth_texture,
        vec2i(fragment_position.xy),
        0,
    ).x;

    if depth <= 0.0 {
        return SURFACE_INFINITY;
    }

    let ndc = fma(
        fragment_position.xy,
        frame.viewport.zw * vec2f(2.0, -2.0),
        vec2f(-1.0, 1.0),
    );
    let view = homogeneous_point(
        frame.inv_proj * vec4f(ndc, depth, 1.0)
    );
    let world = transform_point(frame.inv_view, view);
    let local = transform_point(model.world_to_model, world);

    return dot(local - ray.origin, ray.direction);
}

fn grid_transparent_output(
    in: SurfaceVsOut,
    ray: SurfaceRay,
    first: SurfaceHit,
) -> OitOutput {
    var transmittance = 1.0;
    var premultiplied = vec3f(0.0);
    var depth = 0.0;
    var contributed = false;
    var hit = first;
    // The first crossing is an entry (or a clip cap inside the solid), so the
    // ray is inside and the next crossing is an exit; they alternate.
    var inside = true;
    var budget = representation.options.y;
    let limit_t = surface_opaque_limit(in.position, ray);
    // Each further crossing re-marches the grid, which multiplies the cost
    // of a translucent surface; only a converged output pays for the walls
    // behind the nearest one, an interactive frame keeps the nearest crossing.
    let layers = select(
        1u,
        SURFACE_TRANSPARENT_LAYERS,
        frame.temporal.z > 0.5,
    );

    for (
        var layer = 0u;
        layer < layers;
        layer++
    ) {
        let shaded = surface_transparent_layer(in, hit);

        if shaded.visible {
            let alpha = clamp(shaded.opacity, 0.0, 1.0);

            if !contributed {
                depth = shaded.depth;
                contributed = true;
            }

            premultiplied += transmittance * alpha * shaded.color;
            transmittance *= 1.0 - alpha;
        }

        if layer + 1u == layers ||
            transmittance < SURFACE_LAYER_TRANSMITTANCE_FLOOR {
            break;
        }

        let from_t = dot(
            hit.local_position - ray.origin,
            ray.direction,
        );

        hit = grid_next_crossing(
            ray,
            from_t,
            limit_t,
            inside,
            &budget,
        );

        if !hit.valid {
            break;
        }

        inside = !inside;
    }

    if !contributed {
        discard;
    }

    let opacity = 1.0 - transmittance;

    return weighted_transparency(
        premultiplied / max(opacity, 1.0e-6),
        opacity,
        depth,
    );
}
