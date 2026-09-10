// Optional shared interpolation result for repeatedly consumed rigid transforms.

struct RigidInstanceGpu {
    translation_scale: vec4f,
    orientation: vec4f,
}

struct InstanceTimelineConfig {
    values: vec4f,
    counts: vec4u,
}

@group(0) @binding(0) var<storage, read> frame_start: array<RigidInstanceGpu>;
@group(0) @binding(1) var<storage, read> frame_end: array<RigidInstanceGpu>;
@group(0) @binding(2) var<storage, read_write> frame_output: array<RigidInstanceGpu>;
@group(0) @binding(3) var<uniform> timeline_config: InstanceTimelineConfig;

@compute @workgroup_size(64)
fn materialize_instance_timeline(@builtin(global_invocation_id) id: vec3u) {
    let index = id.x;
    if index >= timeline_config.counts.x {
        return;
    }
    let start = frame_start[timeline_config.counts.y + index];
    let end = frame_end[timeline_config.counts.z + index];
    var end_orientation = end.orientation;
    if dot(start.orientation, end_orientation) < 0.0 {
        end_orientation = -end_orientation;
    }
    frame_output[index] = RigidInstanceGpu(
        mix(start.translation_scale, end.translation_scale, timeline_config.values.x),
        normalize(mix(start.orientation, end_orientation, timeline_config.values.x)),
    );
}
