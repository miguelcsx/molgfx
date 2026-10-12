// Discrete beta boundaries and prefix-oriented frames, O(guides) per coordinate state.
fn strand_unit(value: vec3f, fallback: vec3f) -> vec3f {
    if dot(value, value) < 1e-12 { return fallback; }
    return normalize(value);
}

fn strand_perpendicular(value: vec3f) -> vec3f {
    let magnitude = abs(value);
    var axis = vec3f(0.0, 0.0, 1.0);
    if magnitude.x <= magnitude.y && magnitude.x <= magnitude.z { axis = vec3f(1.0, 0.0, 0.0); }
    else if magnitude.y <= magnitude.z { axis = vec3f(0.0, 1.0, 0.0); }
    return strand_unit(cross(value, axis), vec3f(0.0, 0.0, 1.0));
}

fn strand_store(offset: u32, value: vec3f) {
    fits[offset] = value.x; fits[offset + 1u] = value.y; fits[offset + 2u] = value.z;
}

fn strand_load(offset: u32) -> vec3f {
    return vec3f(fits[offset], fits[offset + 1u], fits[offset + 2u]);
}

fn fit_strand(start: u32, count: u32, output: u32) {
    for (var state = 0u; state < 3u; state++) {
        let base = output + state * (count + 1u) * 12u;
        for (var boundary = 0u; boundary <= count; boundary++) {
            var center = vec3f(0.0);
            if boundary == 0u {
                center = coordinate(jobs[start], state) + coordinate(jobs[start + 1u], state) * 0.5 - coordinate(jobs[start + 2u], state) * 0.5;
            } else if boundary == count {
                center = coordinate(jobs[start + count - 1u], state) + coordinate(jobs[start + count - 2u], state) * 0.5 - coordinate(jobs[start + count - 3u], state) * 0.5;
            } else {
                center = (coordinate(jobs[start + boundary - 1u], state) + coordinate(jobs[start + boundary], state)) * 0.5;
            }
            let guide = clamp(boundary, 1u, count - 2u);
            let left = coordinate(jobs[start + guide], state) - coordinate(jobs[start + guide - 1u], state);
            let right = coordinate(jobs[start + guide + 1u], state) - coordinate(jobs[start + guide], state);
            let width = strand_unit(cross(left, right), strand_perpendicular(right)) * select(-1.0, 1.0, guide % 2u == 0u);
            strand_store(base + boundary * 12u, center);
            strand_store(base + boundary * 12u + 4u, width);
        }
        for (var boundary = 0u; boundary < count; boundary++) {
            let offset = base + boundary * 12u;
            let width = strand_load(offset + 4u);
            let span = strand_load(offset) - strand_load(offset + 12u);
            strand_store(offset + 8u, strand_unit(cross(span, width), strand_perpendicular(width)));
        }
        for (var boundary = 1u; boundary < count; boundary++) {
            let offset = base + boundary * 12u;
            if dot(strand_load(offset - 4u), strand_load(offset + 8u)) < 0.0 {
                strand_store(offset + 4u, -strand_load(offset + 4u));
                strand_store(offset + 8u, -strand_load(offset + 8u));
            }
        }
        strand_store(base + count * 12u + 4u, strand_load(base + (count - 1u) * 12u + 4u));
        strand_store(base + count * 12u + 8u, strand_load(base + (count - 1u) * 12u + 8u));
    }
}
