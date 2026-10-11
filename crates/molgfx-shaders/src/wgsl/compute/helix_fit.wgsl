// O(guides) per helix with a fixed PCA iteration budget; one invocation per fit.
@group(0) @binding(0) var<storage, read> current: array<f32>;
@group(0) @binding(1) var<storage, read> previous: array<f32>;
@group(0) @binding(2) var<storage, read> reference: array<f32>;
@group(0) @binding(3) var<storage, read> jobs: array<u32>;
@group(0) @binding(4) var<storage, read_write> fits: array<f32>;
const AXIS_ITERATIONS: u32 = 12u;

fn coordinate(row: u32, frame: u32) -> vec3f {
    let start = row * 3u;
    if frame == 0u { return vec3f(current[start], current[start+1u], current[start+2u]); }
    if frame == 1u { return vec3f(previous[start], previous[start+1u], previous[start+2u]); }
    return vec3f(reference[start], reference[start+1u], reference[start+2u]);
}

@compute @workgroup_size(1)
fn fit_helix(@builtin(global_invocation_id) invocation: vec3u, @builtin(num_workgroups) extent: vec3u) {
    let job = invocation.y * extent.x + invocation.x;
    if job >= jobs[0] { return; }
    let header = 1u + job * 3u;
    let start = jobs[header];
    let count = jobs[header + 1u];
    let output = jobs[header + 2u];
    for (var frame = 0u; frame < 3u; frame++) {
        var center = vec3f(0.0);
        for (var guide = 0u; guide < count; guide++) { center += coordinate(jobs[start+guide], frame); }
        center /= f32(max(count, 1u));
        // Source rows are visited deterministically; the endpoint chord fixes sense.
        let chord = coordinate(jobs[start+count-1u], frame) - coordinate(jobs[start], frame);
        var axis = vec3f(0.0, 0.0, 1.0);
        if dot(chord, chord) > 1e-12 { axis = normalize(chord); }
        for (var iteration = 0u; iteration < AXIS_ITERATIONS; iteration++) {
            var covariance = vec3f(0.0);
            for (var guide = 0u; guide < count; guide++) {
                let relative = coordinate(jobs[start+guide], frame) - center;
                covariance += relative * dot(relative, axis);
            }
            if dot(covariance, covariance) > 1e-12 { axis = normalize(covariance); }
        }
        if dot(axis, chord) < 0.0 { axis = -axis; }
        let offset = output + frame * 8u;
        fits[offset] = center.x; fits[offset+1u] = center.y; fits[offset+2u] = center.z;
        fits[offset+4u] = axis.x; fits[offset+5u] = axis.y; fits[offset+6u] = axis.z;
    }
}
