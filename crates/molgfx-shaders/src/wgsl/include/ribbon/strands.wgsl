// Closed beta blocks gather one cached frame per boundary; vertex cost is constant.
fn ribbon_strand_unit(value: vec3f, fallback: vec3f) -> vec3f {
    if dot(value, value) < 1e-12 { return fallback; }
    return normalize(value);
}

fn ribbon_strand_vector(base: u32, boundary: u32, column: u32) -> vec3f {
    return ribbon_fit_vector(base + boundary * 12u + column);
}

fn ribbon_strand_sample(base: u32, count: u32, interval: u32, column: u32, parameter: f32, curved: bool) -> vec3f {
    let a = ribbon_strand_vector(base, interval, column);
    let b = ribbon_strand_vector(base, interval + 1u, column);
    if !curved { return mix(a, b, parameter); }
    return ribbon_catmull_position(
        ribbon_strand_vector(base, interval - 1u, column), a, b,
        ribbon_strand_vector(base, min(interval + 2u, count - 1u), column), parameter,
    );
}

fn ribbon_strand_vertex(deformation: RibbonDeformation, previous: bool) -> DeformedRibbonVertex {
    let count = deformation.controls.y;
    let interval = deformation.controls.z;
    let base = deformation.controls.x + select(0u, count * 12u, previous);
    let first_height = ribbon_strand_vector(base, interval, 8u);
    let last_height = ribbon_strand_vector(base, interval + 1u, 8u);
    let curved = interval > 0u && interval + 2u < count && abs(dot(first_height, last_height)) <= 0.9;
    let center = ribbon_strand_sample(base, count, interval, 0u, deformation.parameter.x, curved);
    let width = ribbon_strand_unit(ribbon_strand_sample(base, count, interval, 4u, deformation.parameter.x, curved), ribbon_strand_vector(base, interval, 4u));
    let height = ribbon_strand_unit(ribbon_strand_sample(base, count, interval, 8u, deformation.parameter.x, curved), first_height);
    let span = ribbon_strand_vector(base, interval + 1u, 0u) - ribbon_strand_vector(base, interval, 0u);
    let tangent = ribbon_strand_unit(span, vec3f(1.0, 0.0, 0.0));
    let arrow = (deformation.controls.w & 8u) != 0u;
    var taper = vec3f(0.0);
    if arrow && dot(span, span) > 1e-12 { taper = tangent * (abs(deformation.parameter.y) / length(span)); }
    let face = deformation.controls.w & 7u;
    var normal = normalize(width + taper);
    switch face {
        case 1u: { normal = normalize(-width + taper); }
        case 2u: { normal = height; }
        case 3u: { normal = -height; }
        case 4u, 5u: {
            normal = tangent * select(-1.0, 1.0, face == 5u);
        }
        default: {}
    }
    let half_width = deformation.parameter.y * select(1.0, 1.0 - deformation.parameter.x, arrow);
    return DeformedRibbonVertex(center + width * half_width + height * deformation.parameter.z, normal, 1u);
}
