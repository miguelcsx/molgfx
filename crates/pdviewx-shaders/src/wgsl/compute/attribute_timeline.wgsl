// Linear materialization of one scalar/vector frame pair in the shared arena.

struct AttributeTimelineConfig {
    // start, end, output and alpha-word offsets
    offsets: vec4u,
    // total component words
    counts: vec4u,
}

@group(0) @binding(0) var<storage, read_write> attribute_words: array<u32>;
@group(0) @binding(1) var<uniform> attribute_timeline: AttributeTimelineConfig;

@compute @workgroup_size(64)
fn materialize_attribute_timeline(@builtin(global_invocation_id) id: vec3u) {
    if id.x >= attribute_timeline.counts.x {
        return;
    }
    var alpha = bitcast<f32>(attribute_words[attribute_timeline.offsets.w]);
    if attribute_timeline.counts.z != 0u {
        alpha = bitcast<f32>(attribute_timeline.counts.y);
    }
    let start = bitcast<f32>(attribute_words[attribute_timeline.offsets.x + id.x]);
    let end = bitcast<f32>(attribute_words[attribute_timeline.offsets.y + id.x]);
    attribute_words[attribute_timeline.offsets.z + id.x] = bitcast<u32>(mix(start, end, alpha));
}
