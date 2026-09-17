// Adaptive traversal and sign-bracketed refinement over persistent fields.
//
// Hits are accepted only after a real outside-to-inside crossing. This avoids
// the depth terraces caused by treating an outside epsilon band as geometry,
// while keeping empty-space traversal compact enough for the fragment hot path.

const GRID_ROOT_BISECTIONS: u32 = 2u;
// Two samples per smallest grid cell are sufficient to preserve a trilinear
// sign interval; the bracketed root refinement recovers sub-cell depth. A
// quarter-cell floor doubled near-surface texture work without adding a
// representable crossing.
const GRID_STEP_FRACTION: f32 = 0.5;
const GRID_DISTANCE_SAFETY: f32 = 0.9;

fn grid_clip_face(
    ray: SurfaceRay,
    clipped: RepresentationClipInterval,
) -> SurfaceHit {
    if clipped.entry_plane == NO_CLIP_PLANE ||
        clipped.range.x < 0.0 {
        return surface_miss();
    }

    let hit = fma(
        ray.direction,
        vec3f(clipped.range.x),
        ray.origin,
    );

    if grid_level_value(grid_surface_field(hit)) > 0.0 {
        return surface_miss();
    }

    let nearest = surface_nearest_atom(hit);

    if nearest == EMPTY_COMPACT_INDEX {
        return surface_miss();
    }

    return SurfaceHit(
        hit,
        local_clip_normal(clipped.entry_plane),
        nearest,
        true,
        true,
    );
}

/// Converts stored scalar values to an approximate distance used only to skip
/// empty space. Hit acceptance and refinement always use the exact level sign.
fn grid_empty_space_distance(field: f32) -> f32 {
    if representation.options.x != SURFACE_KIND_GAUSSIAN {
        return max(grid_level_value(field), 0.0);
    }

    let density = max(field, 1.0e-8);
    let isolevel = max(representation.surface.y, 1.0e-8);
    let density_radius = sqrt(max(-2.0 * log(density), 0.0));
    let surface_radius = sqrt(max(-2.0 * log(isolevel), 0.0));

    return max(
        (density_radius - surface_radius) * representation.surface.z,
        0.0,
    );
}

fn grid_refine_hit(
    ray: SurfaceRay,
    low_start: f32,
    high_start: f32,
    low_level_start: f32,
    high_level_start: f32,
) -> f32 {
    var low = low_start;
    var high = high_start;

    // Linear interpolation of the signed endpoint values normally lands much
    // closer to the zero than a midpoint. Clamp it away from either endpoint
    // so a poorly scaled distance estimate still shrinks the bracket.
    let fraction = clamp(
        low_level_start /
            max(low_level_start - high_level_start, 1.0e-8),
        0.1,
        0.9,
    );
    let candidate = mix(low, high, fraction);
    let candidate_point = fma(
        ray.direction,
        vec3f(candidate),
        ray.origin,
    );
    let candidate_level = grid_level_value(
        grid_surface_field(candidate_point)
    );

    if candidate_level > 0.0 {
        low = candidate;
    } else {
        high = candidate;
    }

    for (
        var iteration = 0u;
        iteration < GRID_ROOT_BISECTIONS;
        iteration++
    ) {
        let middle = (low + high) * 0.5;
        let point = fma(ray.direction, vec3f(middle), ray.origin);

        if grid_level_value(grid_surface_field(point)) > 0.0 {
            low = middle;
        } else {
            high = middle;
        }
    }

    return high;
}

fn grid_resolved_hit(
    ray: SurfaceRay,
    parameter: f32,
) -> SurfaceHit {
    let hit = fma(ray.direction, vec3f(parameter), ray.origin);
    let nearest = surface_nearest_atom(hit);

    if nearest == EMPTY_COMPACT_INDEX {
        return surface_miss();
    }

    let coordinate = grid_coordinate(hit);
    return SurfaceHit(
        hit,
        grid_surface_normal(hit, nearest, coordinate),
        nearest,
        true,
        false,
    );
}

fn intersect_grid_surface(ray: SurfaceRay) -> SurfaceHit {
    let grid_upper = representation.grid_min.xyz +
        representation.grid_cell.xyz *
        vec3f(representation.grid_size.xyz - vec3u(1u));
    let clipped = clipped_local_interval(
        ray,
        ray_box(
            ray.origin,
            ray.inverse_direction,
            representation.grid_min.xyz,
            grid_upper,
        ),
    );
    var current_t = max(clipped.range.x, 0.0);
    let maximum_t = clipped.range.y;

    if current_t > maximum_t {
        return surface_miss();
    }

    if representation.clip_meta.y != 0u {
        let cap = grid_clip_face(ray, clipped);

        if cap.valid {
            return cap;
        }
    }

    let minimum_cell = min(
        representation.grid_cell.x,
        min(representation.grid_cell.y, representation.grid_cell.z),
    );
    let minimum_step = minimum_cell * GRID_STEP_FRACTION;
    let gaussian_maximum_step = minimum_cell * 2.0;
    var previous_t = current_t;
    var previous_level = SURFACE_INFINITY;

    for (
        var step = 0u;
        step < representation.options.y;
        step++
    ) {
        let point = fma(ray.direction, vec3f(current_t), ray.origin);
        let field = grid_surface_field(point);
        let level = grid_level_value(field);

        if level <= 0.0 {
            var root = current_t;

            if previous_level > 0.0 && previous_t < current_t {
                root = grid_refine_hit(
                    ray,
                    previous_t,
                    current_t,
                    previous_level,
                    level,
                );
            }

            return grid_resolved_hit(ray, root);
        }

        previous_t = current_t;
        previous_level = level;
        var advance = max(
            grid_empty_space_distance(field) * GRID_DISTANCE_SAFETY,
            minimum_step,
        );

        if representation.options.x == SURFACE_KIND_GAUSSIAN {
            advance = min(advance, gaussian_maximum_step);
        }

        current_t += advance;

        if current_t > maximum_t {
            break;
        }
    }

    return surface_miss();
}
