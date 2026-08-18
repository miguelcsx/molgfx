// Topology-stable two-frame coordinate interpolation.
//
// One invocation processes one tightly packed coordinate, maximizing
// contiguous storage access across neighboring GPU lanes.

struct TrajectoryUniforms {
    alpha: f32,
    previous_alpha: f32,
    count: u32,
    padding: u32,
}

@group(0) @binding(0) var<storage, read> frame_start: array<f32>;
@group(0) @binding(1) var<storage, read> frame_end: array<f32>;
@group(0) @binding(2) var<storage, read_write> output_positions: array<f32>;
@group(0) @binding(3) var<storage, read_write> previous_positions: array<f32>;
@group(0) @binding(4) var<uniform> trajectory: TrajectoryUniforms;

/// Interpolates one coordinate for the current and previous samples.
fn interpolate_coordinate(index: u32) {
    let start = frame_start[index];
    let delta = frame_end[index] - start;

    output_positions[index] = fma(delta, trajectory.alpha, start);
    previous_positions[index] = fma(delta, trajectory.previous_alpha, start);
}

@compute @workgroup_size(64)
fn interpolate_trajectory(@builtin(global_invocation_id) id: vec3u) {
    let coordinate = id.x;

    if coordinate >= trajectory.count * 3u {
        return;
    }

    interpolate_coordinate(coordinate);
}
