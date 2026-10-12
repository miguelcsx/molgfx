// Independently splined sheet borders, gathered from live guide/plane rows.
const RIBBON_EDGE_ANCHOR: u32 = 9u;
const RIBBON_BORDER_SIDES: u32 = 16u;
const RIBBON_BORDER_TAU: f32 = 6.283185307179586;
const RIBBON_BORDER_CAP_START: u32 = 256u;
const RIBBON_BORDER_CAP_END: u32 = 512u;

fn ribbon_edge_unit(value: vec3f, fallback: vec3f) -> vec3f {
    let squared = dot(value, value);
    if squared <= 1e-12 { return fallback; }
    return value * inverseSqrt(squared);
}

fn ribbon_edge_width(previous: vec3f, center: vec3f, next: vec3f, cue: vec3f) -> vec3f {
    let tangent = ribbon_edge_unit(next - previous, vec3f(0.0, 0.0, 1.0));
    let projected = cue - tangent * dot(cue, tangent);
    if dot(projected, projected) > 1e-12 { return normalize(projected); }
    let bend = previous + next - center * 2.0;
    let curvature = bend - tangent * dot(bend, tangent);
    if dot(curvature, curvature) > 1e-12 { return normalize(curvature); }
    let absolute = abs(tangent);
    var seed = vec3f(0.0, 0.0, 1.0);
    if absolute.x <= absolute.y && absolute.x <= absolute.z { seed = vec3f(1.0, 0.0, 0.0); }
    else if absolute.y <= absolute.z { seed = vec3f(0.0, 1.0, 0.0); }
    return ribbon_edge_unit(seed - tangent * dot(seed, tangent), vec3f(1.0, 0.0, 0.0));
}

fn ribbon_edge_vertex(deformation: RibbonDeformation, previous: bool) -> DeformedRibbonVertex {
    let offset = bitcast<u32>(deformation.parameter.y);
    var left: array<vec3f, 4>;
    var right: array<vec3f, 4>;
    for (var guide = 0u; guide < 4u; guide++) {
        let center = ribbon_coordinate(deformation.controls[guide], previous);
        let before = ribbon_coordinate(bitcast<u32>(ribbon_radius_sources[offset + guide * 2u]), previous);
        let after = ribbon_coordinate(bitcast<u32>(ribbon_radius_sources[offset + guide * 2u + 1u]), previous);
        let cue_row = bitcast<u32>(ribbon_radius_sources[offset + 8u + guide]);
        var cue = vec3f(0.0);
        if cue_row != 0xffffffffu { cue = ribbon_coordinate(cue_row, previous) - center; }
        let width = ribbon_edge_width(before, center, after, cue)
            * ribbon_radius_sources[offset + 12u + guide] * ribbon_radius_sources[offset + 16u];
        left[guide] = center - width;
        right[guide] = center + width;
    }
    let t = deformation.parameter.x;
    let a = ribbon_catmull_position(left[0], left[1], left[2], left[3], t);
    let b = ribbon_catmull_position(right[0], right[1], right[2], right[3], t);
    let da = ribbon_catmull_tangent(left[0], left[1], left[2], left[3], t);
    let db = ribbon_catmull_tangent(right[0], right[1], right[2], right[3], t);
    let across = ribbon_edge_unit(b - a, vec3f(1.0, 0.0, 0.0));
    let encoded = bitcast<u32>(deformation.parameter.z);
    let lane = encoded & 255u;
    if lane < 2u {
        return DeformedRibbonVertex(select(a, b, lane == 1u),
            ribbon_edge_unit(cross(da + db, across), vec3f(0.0, 0.0, 1.0)), 1u);
    }
    var side = (lane - 2u) / RIBBON_BORDER_SIDES;
    if lane >= 34u { side = lane - 34u; }
    let tangent = ribbon_edge_unit(select(da, db, side == 1u), vec3f(0.0, 0.0, 1.0));
    let center = select(a, b, side == 1u);
    let radial = ribbon_edge_unit(across - tangent * dot(across, tangent), vec3f(1.0, 0.0, 0.0));
    let binormal = cross(tangent, radial);
    let angle = RIBBON_BORDER_TAU * f32((lane - 2u) % RIBBON_BORDER_SIDES) / f32(RIBBON_BORDER_SIDES);
    var normal = radial * cos(angle) + binormal * sin(angle);
    var position = center + normal * ribbon_radius_sources[offset + 17u];
    if lane >= 34u { position = center; }
    let cap = encoded & ~255u;
    if cap == RIBBON_BORDER_CAP_START { normal = -tangent; }
    else if cap == RIBBON_BORDER_CAP_END { normal = tangent; }
    return DeformedRibbonVertex(position, normal, 1u);
}
