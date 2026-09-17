// Shared stackless traversal policy for quality atom and bond hierarchies.

const QUALITY_TRAVERSAL_END: u32 = 0xffffffffu;

fn quality_node_interval(
    origin: vec3f,
    inverse_direction: vec3f,
    node: BvhNode,
    padding: f32,
) -> vec2f {
    return box_interval(
        origin,
        inverse_direction,
        node.min_left.xyz - vec3f(padding),
        node.max_radius.xyz + vec3f(padding),
    );
}

fn quality_node_is_missed(interval: vec2f, maximum: f32) -> bool {
    return interval.x > interval.y || interval.y < 0.0 || interval.x > maximum;
}

fn quality_node_count(node: BvhNode) -> u32 {
    return bitcast<u32>(node.min_left.w) >> BVH_COUNT_SHIFT;
}

fn quality_node_first(node: BvhNode) -> u32 {
    return bitcast<u32>(node.min_left.w) & BVH_INDEX_MASK;
}

fn quality_escape_base(index_count: u32, node_count: u32) -> u32 {
    return index_count - node_count;
}
