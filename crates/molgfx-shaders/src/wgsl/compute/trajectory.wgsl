// Topology-stable coordinate interpolation: linear between two frames, or cubic
// Hermite over four. The Hermite tangents are folded into four blend weights on
// the CPU, so the branch below is a weighted sum of the resident frames.
//
// One invocation processes one tightly packed coordinate, maximizing
// contiguous storage access across neighboring GPU lanes.

struct TrajectoryUniforms {
    alpha: f32,
    previous_alpha: f32,
    count: u32,
    // 0 = linear (two frames), 1 = cubic Hermite (four frames).
    mode: u32,
    // Weights over [before, start, end, after] at the current and previous samples.
    weights: vec4f,
    previous_weights: vec4f,
}

@group(0) @binding(0) var<storage, read> frame_start: array<f32>;
@group(0) @binding(1) var<storage, read> frame_end: array<f32>;
@group(0) @binding(2) var<storage, read_write> output_positions: array<f32>;
@group(0) @binding(3) var<storage, read_write> previous_positions: array<f32>;
@group(0) @binding(4) var<uniform> trajectory: TrajectoryUniforms;
@group(0) @binding(5) var<storage, read> frame_before: array<f32>;
@group(0) @binding(6) var<storage, read> frame_after: array<f32>;

fn blend(weights: vec4f, frames: vec4f) -> f32 {
    return fma(weights.w, frames.w, fma(weights.z, frames.z, fma(weights.y, frames.y, weights.x * frames.x)));
}

/// Cubic Hermite for the current and previous samples; the previous weights keep
/// the motion vector on the same spline.
fn hermite_coordinate(index: u32) {
    let frames = vec4f(
        frame_before[index],
        frame_start[index],
        frame_end[index],
        frame_after[index],
    );

    output_positions[index] = blend(trajectory.weights, frames);
    previous_positions[index] = blend(trajectory.previous_weights, frames);
}

/// Interpolates one coordinate for the current and previous samples.
fn interpolate_coordinate(index: u32) {
    if trajectory.mode == 1u {
        hermite_coordinate(index);
        return;
    }
    let start = frame_start[index];
    let delta = frame_end[index] - start;

    output_positions[index] = fma(delta, trajectory.alpha, start);
    previous_positions[index] = fma(delta, trajectory.previous_alpha, start);
}

@compute @workgroup_size(64)
fn interpolate_trajectory(
    @builtin(global_invocation_id) id: vec3u,
    @builtin(num_workgroups) groups: vec3u,
) {
    let coordinate = id.x + id.y * groups.x * 64u;

    if coordinate >= trajectory.count * 3u {
        return;
    }

    interpolate_coordinate(coordinate);
}
