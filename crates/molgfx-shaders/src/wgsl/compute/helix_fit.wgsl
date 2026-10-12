// Sequence-order linear regression in O(guides); one invocation per helix.
@group(0) @binding(0) var<storage, read> current: array<f32>;
@group(0) @binding(1) var<storage, read> previous: array<f32>;
@group(0) @binding(2) var<storage, read> reference: array<f32>;
@group(0) @binding(3) var<storage, read> jobs: array<u32>;
@group(0) @binding(4) var<storage, read_write> fits: array<f32>;

fn coordinate(row: u32, frame: u32) -> vec3f {
    let start = row * 3u;
    if frame == 0u { return vec3f(current[start], current[start+1u], current[start+2u]); }
    if frame == 1u { return vec3f(previous[start], previous[start+1u], previous[start+2u]); }
    return vec3f(reference[start], reference[start+1u], reference[start+2u]);
}

//!include "include/ribbon/strand_fit.wgsl"

@compute @workgroup_size(1)
fn fit_helix(@builtin(global_invocation_id) invocation: vec3u, @builtin(num_workgroups) extent: vec3u) {
    let job = invocation.y * extent.x + invocation.x;
    if job >= jobs[0] { return; }
    let header = 1u + job * 3u;
    let start = jobs[header];
    let count = jobs[header + 1u];
    let output = jobs[header + 2u];
    if (count & 0x80000000u) != 0u {
        fit_strand(start, count & 0x7fffffffu, output);
        return;
    }
    for (var frame = 0u; frame < 3u; frame++) {
        var center = vec3f(0.0);
        for (var guide = 0u; guide < count; guide++) { center += coordinate(jobs[start+guide], frame); }
        center /= f32(max(count, 1u));
        let midpoint = (f32(count) - 1.0) * 0.5;
        var numerator = vec3f(0.0);
        var denominator = 0.0;
        for (var guide = 0u; guide < count; guide++) {
            let offset = f32(guide) - midpoint;
            numerator += (coordinate(jobs[start+guide], frame) - center) * offset;
            denominator += offset * offset;
        }
        let axis = numerator / max(denominator, 1e-12);
        let offset = output + frame * 8u;
        fits[offset] = center.x; fits[offset+1u] = center.y; fits[offset+2u] = center.z;
        fits[offset+4u] = axis.x; fits[offset+5u] = axis.y; fits[offset+6u] = axis.z;
    }
}
