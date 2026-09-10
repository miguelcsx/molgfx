// Connected-component filtering for one resident sampled-field working set.
// Parents and statistics are persistent scratch owned by the surface slot.

struct ComponentConfig {
    dimensions: vec4u,
    metric: vec4u,
    values: vec4f,
}

@group(1) @binding(0) var source_field: texture_3d<f32>;
@group(1) @binding(1) var filtered_field: texture_storage_3d<r32float, write>;
@group(1) @binding(2) var<storage, read_write> parents: array<atomic<u32>>;
@group(1) @binding(3) var<storage, read_write> statistics: array<atomic<u32>>;
@group(1) @binding(4) var<uniform> config: ComponentConfig;

const EMPTY_COMPONENT: u32 = 0xFFFFFFFFu;

fn voxel_index(coordinate: vec3u) -> u32 {
    return coordinate.x + config.dimensions.x *
        (coordinate.y + config.dimensions.y * coordinate.z);
}

fn in_bounds(coordinate: vec3i) -> bool {
    return all(coordinate >= vec3i(0)) &&
        all(coordinate < vec3i(config.dimensions.xyz));
}

fn occupied(coordinate: vec3i) -> bool {
    if !in_bounds(coordinate) { return false; }
    let field = textureLoad(source_field, coordinate, 0).x;
    return select(field <= config.values.x, field >= config.values.x, config.metric.y != 0u);
}

fn root_of(start: u32) -> u32 {
    var root = start;
    for (var step = 0u; step < 32u; step += 1u) {
        let parent = atomicLoad(&parents[root]);
        if parent == root || parent == EMPTY_COMPONENT { break; }
        root = parent;
    }
    return root;
}

fn unite(left: u32, right: u32) {
    var a = root_of(left);
    var b = root_of(right);
    for (var step = 0u; step < 32u && a != b; step += 1u) {
        let high = max(a, b);
        let low = min(a, b);
        atomicMin(&parents[high], low);
        a = root_of(low);
        b = root_of(high);
    }
}

@compute @workgroup_size(4, 4, 4)
fn cs_component_initialize(@builtin(global_invocation_id) coordinate: vec3u) {
    if any(coordinate >= config.dimensions.xyz) { return; }
    let index = voxel_index(coordinate);
    atomicStore(&parents[index], select(EMPTY_COMPONENT, index, occupied(vec3i(coordinate))));
    atomicStore(&statistics[index * 2u], 0u);
    atomicStore(&statistics[index * 2u + 1u], 0u);
}

@compute @workgroup_size(4, 4, 4)
fn cs_component_union(@builtin(global_invocation_id) coordinate: vec3u) {
    if any(coordinate >= config.dimensions.xyz) { return; }
    let index = voxel_index(coordinate);
    if atomicLoad(&parents[index]) == EMPTY_COMPONENT { return; }
    let signed = vec3i(coordinate);
    let neighbours = array<vec3i, 3>(
        signed - vec3i(1, 0, 0),
        signed - vec3i(0, 1, 0),
        signed - vec3i(0, 0, 1),
    );
    for (var side = 0u; side < 3u; side += 1u) {
        if occupied(neighbours[side]) {
            unite(index, voxel_index(vec3u(neighbours[side])));
        }
    }
}

@compute @workgroup_size(4, 4, 4)
fn cs_component_compress(@builtin(global_invocation_id) coordinate: vec3u) {
    if any(coordinate >= config.dimensions.xyz) { return; }
    let index = voxel_index(coordinate);
    let parent = atomicLoad(&parents[index]);
    if parent == EMPTY_COMPONENT || parent == index { return; }
    atomicStore(&parents[index], atomicLoad(&parents[parent]));
}

@compute @workgroup_size(4, 4, 4)
fn cs_component_measure(@builtin(global_invocation_id) coordinate: vec3u) {
    if any(coordinate >= config.dimensions.xyz) { return; }
    let index = voxel_index(coordinate);
    let root = atomicLoad(&parents[index]);
    if root == EMPTY_COMPONENT { return; }
    atomicAdd(&statistics[root * 2u], 1u);
    let point = vec3i(coordinate);
    let neighbours = array<vec3i, 6>(
        point - vec3i(1, 0, 0), point + vec3i(1, 0, 0),
        point - vec3i(0, 1, 0), point + vec3i(0, 1, 0),
        point - vec3i(0, 0, 1), point + vec3i(0, 0, 1),
    );
    var exposed = 0u;
    for (var side = 0u; side < 6u; side += 1u) {
        exposed += select(1u, 0u, occupied(neighbours[side]));
    }
    atomicAdd(&statistics[root * 2u + 1u], exposed);
}

fn component_kept(root: u32) -> bool {
    let voxels = atomicLoad(&statistics[root * 2u]);
    let faces = atomicLoad(&statistics[root * 2u + 1u]);
    if config.metric.x == 1u {
        return f32(faces) * config.values.y * config.values.y >= config.values.z;
    }
    if config.metric.x == 2u {
        return f32(voxels) * config.values.y * config.values.y * config.values.y >= config.values.z;
    }
    if config.metric.x == 3u { return voxels >= config.metric.z; }
    return config.metric.x != 4u;
}

@compute @workgroup_size(4, 4, 4)
fn cs_component_filter(@builtin(global_invocation_id) coordinate: vec3u) {
    if any(coordinate >= config.dimensions.xyz) { return; }
    let index = voxel_index(coordinate);
    let parent = atomicLoad(&parents[index]);
    let keep = parent != EMPTY_COMPONENT && component_kept(parent);
    let source = textureLoad(source_field, vec3i(coordinate), 0).x;
    let outside = select(64.0, 0.0, config.metric.y != 0u);
    textureStore(filtered_field, vec3i(coordinate), vec4f(select(outside, source, keep)));
}
