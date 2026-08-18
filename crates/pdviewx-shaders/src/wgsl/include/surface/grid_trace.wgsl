// Ray marching and hit refinement over the persistent SES grid.
//
// The march is bounded by the field's own Lipschitz-safe step, so an empty
// region is crossed in few samples rather than at a fixed rate. Refinement
// runs only after a sign change, keeping the per-pixel cost proportional to
// the distance travelled rather than to the grid resolution.

fn grid_clip_face(
    ray: SurfaceRay,
    clipped: RepresentationClipInterval,
) -> SurfaceHit {
    if clipped.entry_plane ==
        NO_CLIP_PLANE ||
        clipped.range.x < 0.0 {
        return surface_miss();
    }

    let hit =
        fma(
            ray.direction,
            vec3f(clipped.range.x),
            ray.origin,
        );

    if grid_surface_field(hit) >
        representation.surface.y {
        return surface_miss();
    }

    let coordinate =
        grid_coordinate(hit);

    let nearest =
        surface_provenance_at(
            hit,
            coordinate,
        );

    if nearest ==
        EMPTY_COMPACT_INDEX {
        return surface_miss();
    }

    return SurfaceHit(
        hit,
        local_clip_normal(
            clipped.entry_plane
        ),
        nearest,
        true,
        true,
    );
}

/// Refines only a true outside/inside bracket.
fn refine_grid_hit(
    origin: vec3f,
    direction: vec3f,
    low_start: f32,
    high_start: f32,
) -> f32 {
    var low =
        low_start;

    var high =
        high_start;

    for (
        var iteration = 0u;
        iteration < 6u;
        iteration++
    ) {
        let middle =
            (low + high) * 0.5;

        if grid_surface_field(
            fma(
                direction,
                vec3f(middle),
                origin,
            )
        ) > representation.surface.y {
            low = middle;
        } else {
            high = middle;
        }
    }

    return high;
}

fn intersect_grid_surface(
    ray: SurfaceRay,
) -> SurfaceHit {
    let root =
        bvh_nodes[0];

    let padding =
        vec3f(
            representation.surface.x +
            abs(representation.surface.y)
        );

    let root_interval =
        ray_box(
            ray.origin,
            ray.inverse_direction,
            root.min_left.xyz -
                padding,
            root.max_radius.xyz +
                padding,
        );

    let clipped =
        clipped_local_interval(
            ray,
            root_interval,
        );

    let maximum_t =
        clipped.range.y;

    var t =
        max(
            clipped.range.x,
            0.0,
        );

    if t > maximum_t {
        return surface_miss();
    }

    if representation.clip_meta.y != 0u {
        let cap =
            grid_clip_face(
                ray,
                clipped,
            );

        if cap.valid {
            return cap;
        }
    }

    var previous_t = t;
    var previous_distance =
        SURFACE_INFINITY;

    let max_steps =
        representation.options.y;

    for (
        var step = 0u;
        step < max_steps;
        step++
    ) {
        let point =
            fma(
                ray.direction,
                vec3f(t),
                ray.origin,
            );

        let distance =
            grid_surface_field(point) -
            representation.surface.y;

        if distance <=
            representation.surface.z {
            var refined = t;

            // The old bisection produced no improvement when the current
            // sample was still outside. Avoid up to 48 unnecessary loads.
            if distance <= 0.0
                && previous_distance > 0.0
                && previous_t < t {
                refined =
                    refine_grid_hit(
                        ray.origin,
                        ray.direction,
                        previous_t,
                        t,
                    );
            }

            let hit =
                fma(
                    ray.direction,
                    vec3f(refined),
                    ray.origin,
                );

            let coordinate =
                grid_coordinate(hit);

            let nearest =
                surface_provenance_at(
                    hit,
                    coordinate,
                );

            if nearest ==
                EMPTY_COMPACT_INDEX {
                return surface_miss();
            }

            return SurfaceHit(
                hit,

                grid_surface_normal(
                    hit,
                    nearest,
                    coordinate,
                ),

                nearest,
                true,
                false,
            );
        }

        previous_t =
            t;

        previous_distance =
            distance;

        t += max(
            distance *
                SURFACE_MARCH_SAFETY,
            representation.surface.w,
        );

        if t > maximum_t {
            break;
        }
    }

    return surface_miss();
}
