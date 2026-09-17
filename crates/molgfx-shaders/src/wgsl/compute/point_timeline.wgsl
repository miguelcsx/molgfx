// Optional shared interpolation result for repeatedly consumed point positions.

struct PointTimelineConfig {
    values: vec4f,
    counts: vec4u,
}

@group(0) @binding(0) var<storage, read> frame_start: array<u32>;
@group(0) @binding(1) var<storage, read> frame_end: array<u32>;
@group(0) @binding(2) var<storage, read_write> frame_output: array<u32>;
@group(0) @binding(3) var<uniform> timeline_config: PointTimelineConfig;

@compute @workgroup_size(64)
fn materialize_point_timeline(@builtin(global_invocation_id) id: vec3u) {
    let index = id.x;
    if index >= timeline_config.counts.x {
        return;
    }
    let base = index * 3u;
    let start = bitcast<vec3f>(vec3u(
        frame_start[base],
        frame_start[base + 1u],
        frame_start[base + 2u],
    ));
    let end = bitcast<vec3f>(vec3u(
        frame_end[base],
        frame_end[base + 1u],
        frame_end[base + 2u],
    ));
    let value = bitcast<vec3u>(mix(start, end, timeline_config.values.x));
    frame_output[base] = value.x;
    frame_output[base + 1u] = value.y;
    frame_output[base + 2u] = value.z;
}
