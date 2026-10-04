// Exact optical integration through constant integer-labelled cells.
struct SegmentIntegration {
    accumulated: vec4f,
    representative_t: f32,
    label: u32,
}

fn integrate_segments(ray: SegmentationRay, interval: vec2f) -> SegmentIntegration {
    var t = interval.x;
    let entry = fma(ray.voxel_direction, vec3f(t), ray.voxel_origin);
    var cell = vec3i(floor(entry + vec3f(0.5)));
    let direction = sign(ray.voxel_direction);
    let cell_step = vec3i(direction);
    var crossing = (vec3f(cell) + direction * 0.5 - ray.voxel_origin)
        * ray.voxel_inverse_direction;
    // A ray entering exactly on a boundary belongs to the cell ahead of it.
    for (var axis = 0u; axis < 3u; axis++) {
        if ray.voxel_direction[axis] == 0.0 {
            crossing[axis] = SEGMENT_INFINITY;
        } else if crossing[axis] <= t {
            cell[axis] += cell_step[axis];
            // Derive from the integer boundary instead of accumulating
            // thousands of rounded increments on small affine voxels.
            crossing[axis] = (f32(cell[axis]) + direction[axis] * 0.5
                - ray.voxel_origin[axis]) * ray.voxel_inverse_direction[axis];
        }
    }

    var accumulated = vec4f(0.0);
    var representative_t = interval.x;
    var representative_label = 0u;
    var representative_found = false;
    var cached_label = 0u;
    var cached_style = absent_style();
    var cache_valid = false;

    // Integrate each constant-label cell over its exact world-space length.
    // This avoids stochastic coverage and cannot skip a thin labelled voxel.
    while t < interval.y && accumulated.a < SEGMENT_TERMINATION_ALPHA {
        let next_crossing = min(min(crossing.x, crossing.y), crossing.z);
        let next_t = min(next_crossing, interval.y);
        let label = segment_label_texel(cell);
        if !cache_valid || label != cached_label {
            cached_label = label;
            cached_style = sample_style(label);
            cache_valid = true;
        }
        if cached_style.found && cached_style.opacity > SEGMENT_OPACITY_EPSILON {
            let extinction = cached_style.opacity * volume.sampling.x /
                volume.sampling.z;
            let alpha = 1.0 - exp(-extinction * max(next_t - t, 0.0));
            let contribution = (1.0 - accumulated.a) * alpha;
            if contribution > 0.0 {
                let previous_alpha = accumulated.a;
                accumulated += vec4f(contribution * srgb_to_linear(cached_style.color), contribution);
                // Faint visible regions still need a real depth and label.
                if !representative_found {
                    representative_t = t;
                    representative_label = label;
                    representative_found = true;
                }
                if previous_alpha < SEGMENT_REPRESENTATIVE_ALPHA &&
                    accumulated.a >= SEGMENT_REPRESENTATIVE_ALPHA {
                    representative_t = t - log((1.0 - SEGMENT_REPRESENTATIVE_ALPHA) /
                        (1.0 - previous_alpha)) / extinction;
                    representative_label = label;
                }
            }
        }
        if next_t >= interval.y {
            break;
        }
        for (var axis = 0u; axis < 3u; axis++) {
            if crossing[axis] <= next_crossing {
                cell[axis] += cell_step[axis];
                // Derive from the integer boundary instead of accumulating
            // thousands of rounded increments on small affine voxels.
            crossing[axis] = (f32(cell[axis]) + direction[axis] * 0.5
                - ray.voxel_origin[axis]) * ray.voxel_inverse_direction[axis];
            }
        }
        t = next_t;
    }
    return SegmentIntegration(accumulated, representative_t, representative_label);
}
