// Conservative thin solids for zero-thickness circle and square casters.

fn shadow_primitive_size(size: vec3f) -> vec3f {
    if SHADOW_PRIMITIVE_KIND ==
        SHADOW_KIND_PARTICLE_CIRCLE {
        let diameter = max(size.x, size.y);
        return vec3f(
            diameter,
            diameter,
            max(diameter * 0.01, 1.0e-4),
        );
    }
    if SHADOW_PRIMITIVE_KIND ==
        SHADOW_KIND_PARTICLE_SQUARE {
        return vec3f(
            size.xy,
            max(min(size.x, size.y) * 0.01, 1.0e-4),
        );
    }
    return size;
}
