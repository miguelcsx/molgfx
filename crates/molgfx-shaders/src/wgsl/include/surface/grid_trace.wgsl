// Adaptive traversal and sign-bracketed refinement over persistent fields.
//
// Hits are accepted only after a real outside-to-inside crossing. This avoids
// the depth terraces caused by treating an outside epsilon band as geometry,
// while keeping empty-space traversal compact enough for the fragment hot path.

// Refinement narrows the sign bracket until it is a small fraction of the
// smallest cell, independent of how wide the traversal step was. A bracket is
// at most two cells wide, so six halvings after the interpolated candidate
// reach about 0.03 cell; the cap bounds the per-pixel cost.
const GRID_ROOT_BISECTIONS: u32 = 6u;
const GRID_ROOT_TOLERANCE_CELLS: f32 = 0.03;
// Samples are at most half a cell apart where the field is near the level.
// Along a ray a trilinear cell is a cubic, so this does not prove that every
// sign interval is seen: a sliver thinner than the sample spacing can lie
// between two samples. Measured against an exact cell-by-cell tracer the
// misses are about 1e-4 of hits, mostly grazing silhouette rays. A
// quarter-cell floor doubled near-surface texture work for no measurable gain.
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
///
/// For Gaussian fields this inverts a single unit Gaussian of width
/// `surface.z` (sigma, not radius * sigma). That is not a conservative bound
/// for a sum of Gaussians or for atoms with a radius other than one. Safety
/// comes from the two-cell step cap in `intersect_grid_surface` and from the
/// sign-bracketed acceptance, not from this estimate.
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

    let minimum_cell = min(
        representation.grid_cell.x,
        min(representation.grid_cell.y, representation.grid_cell.z),
    );
    let tolerance = minimum_cell * GRID_ROOT_TOLERANCE_CELLS;

    for (
        var iteration = 0u;
        iteration < GRID_ROOT_BISECTIONS;
        iteration++
    ) {
        if high - low <= tolerance {
            break;
        }

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

/// The clipped interval a ray spends inside the field's bounding box.
fn grid_clipped_span(ray: SurfaceRay) -> RepresentationClipInterval {
    let grid_upper = representation.grid_min.xyz +
        representation.grid_cell.xyz *
        vec3f(representation.grid_size.xyz - vec3u(1u));

    return clipped_local_interval(
        ray,
        ray_box(
            ray.origin,
            ray.inverse_direction,
            representation.grid_min.xyz,
            grid_upper,
        ),
    );
}

/// Marches from `start_t` to the next outside-to-inside crossing.
///
/// `budget` counts the samples still available and is spent as the march
/// runs, so a caller that continues past a hit shares one bound across every
/// search instead of multiplying it.
fn grid_march_entry(
    ray: SurfaceRay,
    start_t: f32,
    maximum_t: f32,
    budget: ptr<function, u32>,
) -> SurfaceHit {
    var current_t = start_t;
    let minimum_cell = min(
        representation.grid_cell.x,
        min(representation.grid_cell.y, representation.grid_cell.z),
    );
    let minimum_step = minimum_cell * GRID_STEP_FRACTION;
    let gaussian_maximum_step = minimum_cell * 2.0;
    var previous_t = current_t;
    var previous_level = SURFACE_INFINITY;

    while *budget > 0u {
        *budget -= 1u;
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

/// Marches from inside the field to the next inside-to-outside crossing.
///
/// The returned normal is reversed so it faces the viewer, as the boundary
/// seen from inside the solid. Inside, a signed-distance level bounds the
/// distance to the boundary, so the step grows with depth; a Gaussian density
/// has no such bound and keeps the two-cell cap used for entry.
fn grid_march_exit(
    ray: SurfaceRay,
    start_t: f32,
    maximum_t: f32,
    budget: ptr<function, u32>,
) -> SurfaceHit {
    let minimum_cell = min(
        representation.grid_cell.x,
        min(representation.grid_cell.y, representation.grid_cell.z),
    );
    let minimum_step = minimum_cell * GRID_STEP_FRACTION;
    // The ray starts on the boundary it entered by, so the first sample is
    // one step in; sampling the start itself could report that same boundary.
    var previous_t = start_t;
    var current_t = start_t + minimum_step;

    while *budget > 0u && current_t <= maximum_t {
        *budget -= 1u;
        let level = grid_level_value(
            grid_surface_field(fma(ray.direction, vec3f(current_t), ray.origin))
        );

        if level > 0.0 {
            var low = previous_t;
            var high = current_t;

            for (
                var iteration = 0u;
                iteration < GRID_ROOT_BISECTIONS;
                iteration++
            ) {
                let middle = (low + high) * 0.5;
                let inside = grid_level_value(
                    grid_surface_field(fma(ray.direction, vec3f(middle), ray.origin))
                ) <= 0.0;
                low = select(low, middle, inside);
                high = select(middle, high, inside);
            }

            // Resolve on the outside end of the bracket: the next entry march
            // starts here, and an inside sample would report this same wall
            // again as an entry.
            let boundary = grid_resolved_hit(ray, high);

            return SurfaceHit(
                boundary.local_position,
                -boundary.local_normal,
                boundary.compact_index,
                boundary.valid,
                false,
            );
        }

        previous_t = current_t;
        var advance = minimum_cell * 2.0;

        if representation.options.x != SURFACE_KIND_GAUSSIAN {
            advance = max(-level * GRID_DISTANCE_SAFETY, minimum_step);
        }

        current_t += advance;
    }

    return surface_miss();
}

/// The next boundary crossing after `from_t`: an exit when the ray is inside
/// the field, an entry when it is outside. Callers alternate the two.
///
/// `limit_t` is the ray parameter of the opaque surface along this ray; no
/// crossing beyond it is reported, because opaque geometry hides it.
fn grid_next_crossing(
    ray: SurfaceRay,
    from_t: f32,
    limit_t: f32,
    inside: bool,
    budget: ptr<function, u32>,
) -> SurfaceHit {
    let maximum_t = min(grid_clipped_span(ray).range.y, limit_t);

    if inside {
        return grid_march_exit(ray, from_t, maximum_t, budget);
    }

    return grid_march_entry(ray, from_t, maximum_t, budget);
}

fn intersect_grid_surface(ray: SurfaceRay) -> SurfaceHit {
    let clipped = grid_clipped_span(ray);
    let current_t = max(clipped.range.x, 0.0);
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

    var budget = representation.options.y;

    return grid_march_entry(ray, current_t, maximum_t, &budget);
}
