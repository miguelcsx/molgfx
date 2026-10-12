// Ring geometry gathers topology rows from the shared auxiliary stream.
// The first twelve lanes contain six amplitude/packed-colour ramp stops.
const RING_FULL_TURN: f32 = 6.283185307;

struct RingFrame {
    center: vec3f,
    normal: vec3f,
}

fn ribbon_ring_frame(deformation: RibbonDeformation, previous: bool) -> RingFrame {
    let start = bitcast<u32>(deformation.parameter.x) + 1u;
    let count = u32(deformation.parameter.y);
    var center = vec3f(0.0);
    var normal = vec3f(0.0);
    for (var i = 0u; i < count; i += 1u) {
        center += ribbon_coordinate(bitcast<u32>(ribbon_radius_sources[start + i]), previous);
    }
    center /= f32(count);
    for (var i = 0u; i < count; i += 1u) {
        let a = ribbon_coordinate(bitcast<u32>(ribbon_radius_sources[start + i]), previous) - center;
        let b = ribbon_coordinate(bitcast<u32>(ribbon_radius_sources[start + (i + 1u) % count]), previous) - center;
        normal += cross(a, b);
    }
    if dot(normal, normal) <= 1e-12 { return RingFrame(center, vec3f(0.0)); }
    return RingFrame(center, normalize(normal));
}

fn ribbon_ring_vertex(deformation: RibbonDeformation, previous: bool) -> DeformedRibbonVertex {
    let ring = ribbon_ring_frame(deformation, previous);
    if dot(ring.normal, ring.normal) == 0.0 {
        return DeformedRibbonVertex(ring.center, vec3f(0.0, 0.0, 1.0), 1u);
    }
    let encoded_corner = bitcast<u32>(deformation.parameter.z);
    let corner = encoded_corner & 3u;
    let downward = (encoded_corner & 4u) != 0u;
    let edge = ribbon_coordinate(deformation.controls.x, previous);
    let next = ribbon_coordinate(deformation.controls.y, previous);
    let height = ribbon_radius_sources[bitcast<u32>(deformation.parameter.x)];
    let apex = ring.center + ring.normal * height * select(1.0, -1.0, downward);
    let face_normal = cross(edge - apex, next - apex) * select(1.0, -1.0, downward);
    let position = select(select(apex, edge, corner == 1u), next, corner == 2u);
    if dot(face_normal, face_normal) <= 1e-12 {
        return DeformedRibbonVertex(ring.center, ring.normal, 1u);
    }
    return DeformedRibbonVertex(position, normalize(face_normal), 1u);
}

fn ribbon_ring_color(vertex_id: u32, fallback: vec4f) -> vec4f {
    let deformation = ribbon_deformations[vertex_id];
    if bitcast<u32>(deformation.parameter.w) != 8u { return fallback; }
    let ring = ribbon_ring_frame(deformation, false);
    let start = bitcast<u32>(deformation.parameter.x) + 1u;
    let count = u32(deformation.parameter.y);
    var sine = vec3f(0.0);
    var cosine = vec3f(0.0);
    for (var i = 0u; i < count; i += 1u) {
        let point = ribbon_coordinate(bitcast<u32>(ribbon_radius_sources[start + i]), false) - ring.center;
        let angle = RING_FULL_TURN * f32(i) / f32(count);
        sine += point * sin(angle);
        cosine += point * cos(angle);
    }
    let mean_plane = cross(sine, cosine);
    var pucker_normal = vec3f(0.0);
    if dot(mean_plane, mean_plane) > 1e-12 { pucker_normal = normalize(mean_plane); }
    var amplitude_squared = 0.0;
    for (var i = 0u; i < count; i += 1u) {
        let point = ribbon_coordinate(bitcast<u32>(ribbon_radius_sources[start + i]), false);
        let displacement = dot(point - ring.center, pucker_normal);
        amplitude_squared += displacement * displacement;
    }
    let amplitude = sqrt(amplitude_squared);
    for (var i = 0u; i < 5u; i += 1u) {
        let lower = ribbon_radius_sources[i * 2u];
        let upper = ribbon_radius_sources[i * 2u + 2u];
        if amplitude >= upper { continue; }
        let a = unpack4x8unorm(bitcast<u32>(ribbon_radius_sources[i * 2u + 1u]));
        let b = unpack4x8unorm(bitcast<u32>(ribbon_radius_sources[i * 2u + 3u]));
        return mix(a, b, clamp((amplitude - lower) / (upper - lower), 0.0, 1.0));
    }
    return unpack4x8unorm(bitcast<u32>(ribbon_radius_sources[11u]));
}
