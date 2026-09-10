// GPU-resident temporal occupancy accumulation.

struct OccupancyUniforms {
    model_to_voxel: mat4x4f,
    dimensions: vec4u,
    macro_dimensions: vec4u,
    parameters: vec4f,
    counts: vec4u,
}

@group(0) @binding(0) var<storage, read_write> occupancy: array<atomic<u32>>;
@group(0) @binding(1) var<storage, read> selected_rows: array<u32>;
@group(0) @binding(2) var<storage, read> coordinates: array<f32>;
@group(0) @binding(3) var<uniform> config: OccupancyUniforms;
@group(0) @binding(4) var output_density: texture_storage_3d<r32float, write>;
@group(0) @binding(5) var output_bounds: texture_storage_3d<rg32float, write>;

fn linear_index(id: vec3u, groups: vec3u) -> u32 {
    return id.x + id.y * groups.x * 64u;
}

fn voxel_index(position: vec3u) -> u32 {
    return position.x
        + position.y * config.dimensions.x
        + position.z * config.dimensions.x * config.dimensions.y;
}

fn occupancy_sample(position: vec3i) -> f32 {
    if any(position < vec3i(0)) || any(position >= vec3i(config.dimensions.xyz)) {
        return 0.0;
    }
    return f32(atomicLoad(&occupancy[voxel_index(vec3u(position))]));
}

// Compact tent reconstruction suppresses voxel-scale beads without another
// volume, dispatch or host-visible history. The weights sum to one in the
// interior and the same function feeds both density and conservative bounds.
fn reconstructed_occupancy(position: vec3u) -> f32 {
    let p = vec3i(position);
    let center = occupancy_sample(p) * 0.4;
    let axial = occupancy_sample(p + vec3i(1, 0, 0))
        + occupancy_sample(p + vec3i(-1, 0, 0))
        + occupancy_sample(p + vec3i(0, 1, 0))
        + occupancy_sample(p + vec3i(0, -1, 0))
        + occupancy_sample(p + vec3i(0, 0, 1))
        + occupancy_sample(p + vec3i(0, 0, -1));
    return (center + axial * 0.1) * config.parameters.w;
}

fn load_position(row: u32) -> vec3f {
    let base = row * 3u;
    return vec3f(coordinates[base], coordinates[base + 1u], coordinates[base + 2u]);
}

fn saturating_add(index: u32, increment: u32) {
    var previous = atomicLoad(&occupancy[index]);
    loop {
        let next = min(previous + min(increment, 0xffffffffu - previous), u32(config.parameters.z));
        let exchanged = atomicCompareExchangeWeak(&occupancy[index], previous, next);
        if exchanged.exchanged {
            return;
        }
        previous = exchanged.old_value;
    }
}

@compute @workgroup_size(64)
fn clear_occupancy(
    @builtin(global_invocation_id) id: vec3u,
    @builtin(num_workgroups) groups: vec3u,
) {
    let index = linear_index(id, groups);
    if index < config.counts.y {
        atomicStore(&occupancy[index], 0u);
    }
}

@compute @workgroup_size(64)
fn decay_occupancy(
    @builtin(global_invocation_id) id: vec3u,
    @builtin(num_workgroups) groups: vec3u,
) {
    let index = linear_index(id, groups);
    if index < config.counts.y {
        let value = f32(atomicLoad(&occupancy[index])) * config.parameters.x;
        atomicStore(&occupancy[index], u32(round(value)));
    }
}

@compute @workgroup_size(64)
fn deposit_occupancy(
    @builtin(global_invocation_id) id: vec3u,
    @builtin(num_workgroups) groups: vec3u,
) {
    let selected = linear_index(id, groups);
    if selected >= config.counts.x {
        return;
    }
    let model = load_position(selected_rows[selected]);
    let voxel = (config.model_to_voxel * vec4f(model, 1.0)).xyz;
    let lower_f = floor(voxel);
    let upper_limit = vec3f(config.dimensions.xyz - vec3u(1u));
    if any(lower_f < vec3f(0.0)) || any(lower_f >= upper_limit) {
        return;
    }
    let lower = vec3u(lower_f);
    let fraction = voxel - lower_f;
    for (var z = 0u; z < 2u; z++) {
        for (var y = 0u; y < 2u; y++) {
            for (var x = 0u; x < 2u; x++) {
                let step = vec3u(x, y, z);
                let axis = select(vec3f(1.0) - fraction, fraction, step == vec3u(1u));
                let weight = axis.x * axis.y * axis.z;
                let increment = u32(round(config.parameters.y * weight));
                if increment > 0u {
                    saturating_add(voxel_index(lower + step), increment);
                }
            }
        }
    }
}

@compute @workgroup_size(64)
fn resolve_occupancy(
    @builtin(global_invocation_id) id: vec3u,
    @builtin(num_workgroups) groups: vec3u,
) {
    let index = linear_index(id, groups);
    if index >= config.counts.y {
        return;
    }
    let plane = config.dimensions.x * config.dimensions.y;
    let z = index / plane;
    let rest = index - z * plane;
    let y = rest / config.dimensions.x;
    let x = rest - y * config.dimensions.x;
    let value = reconstructed_occupancy(vec3u(x, y, z));
    textureStore(output_density, vec3u(x, y, z), vec4f(value, 0.0, 0.0, 0.0));
}

@compute @workgroup_size(64)
fn resolve_occupancy_bounds(
    @builtin(global_invocation_id) id: vec3u,
    @builtin(num_workgroups) groups: vec3u,
) {
    let index = linear_index(id, groups);
    if index >= config.counts.z {
        return;
    }
    let macro_plane = config.macro_dimensions.x * config.macro_dimensions.y;
    let macro_z = index / macro_plane;
    let rest = index - macro_z * macro_plane;
    let macro_y = rest / config.macro_dimensions.x;
    let macro_x = rest - macro_y * config.macro_dimensions.x;
    let lower = vec3u(macro_x, macro_y, macro_z) * config.counts.w;
    let upper = min(lower + vec3u(config.counts.w + 1u), config.dimensions.xyz);
    var minimum = 3.402823466e+38;
    var maximum = 0.0;
    for (var z = lower.z; z < upper.z; z++) {
        for (var y = lower.y; y < upper.y; y++) {
            for (var x = lower.x; x < upper.x; x++) {
                let value = reconstructed_occupancy(vec3u(x, y, z));
                minimum = min(minimum, value);
                maximum = max(maximum, value);
            }
        }
    }
    textureStore(
        output_bounds,
        vec3u(macro_x, macro_y, macro_z),
        vec4f(minimum, maximum, 0.0, 0.0),
    );
}
