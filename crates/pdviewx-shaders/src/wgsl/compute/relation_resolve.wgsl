// Branch-free dynamic relation endpoint resolution.
// One pipeline entry point is selected for each homogeneous stream.

//!include "include/quaternion.wgsl"

struct RelationResolverGpu {
    start: vec4f,
    end: vec4f,
    output: vec4u,
    reserved: vec4u,
}

struct InteractionGpu {
    start_width: vec4f,
    end_period: vec4f,
    color: vec4f,
    metadata: vec4u,
    style: vec4f,
    animation: vec4f,
}

struct SourceModel {
    model_to_world: mat4x4f,
}

struct RigidSourceConfig {
    counts: vec4u,
    picking_style: vec4u,
    bounds: vec4f,
}

@group(1) @binding(0) var<storage, read> relation_resolvers: array<RelationResolverGpu>;
@group(1) @binding(1) var<storage, read> start_source_a: array<u32>;
@group(1) @binding(2) var<storage, read> end_source_a: array<u32>;
@group(1) @binding(3) var<storage, read_write> relation_output: array<InteractionGpu>;
@group(1) @binding(4) var<uniform> start_model: SourceModel;
@group(1) @binding(5) var<uniform> end_model: SourceModel;
@group(1) @binding(6) var<storage, read> start_source_b: array<u32>;
@group(1) @binding(7) var<storage, read> end_source_b: array<u32>;
@group(1) @binding(8) var<uniform> start_config: RigidSourceConfig;
@group(1) @binding(9) var<uniform> end_config: RigidSourceConfig;

fn source_vec3_start(row: u32) -> vec3f {
    let base = row * 3u;
    return bitcast<vec3f>(vec3u(start_source_a[base], start_source_a[base + 1u], start_source_a[base + 2u]));
}

fn source_vec3_end(row: u32) -> vec3f {
    let base = row * 3u;
    return bitcast<vec3f>(vec3u(end_source_a[base], end_source_a[base + 1u], end_source_a[base + 2u]));
}

fn source_vec3_start_b(row: u32) -> vec3f {
    let base = row * 3u;
    return bitcast<vec3f>(vec3u(start_source_b[base], start_source_b[base + 1u], start_source_b[base + 2u]));
}

fn source_vec3_end_b(row: u32) -> vec3f {
    let base = row * 3u;
    return bitcast<vec3f>(vec3u(end_source_b[base], end_source_b[base + 1u], end_source_b[base + 2u]));
}

fn world_start(payload: vec4f) -> vec3f { return payload.xyz; }
fn world_end(payload: vec4f) -> vec3f { return payload.xyz; }

fn point_start(payload: vec4f) -> vec3f {
    let row = bitcast<u32>(payload.w);
    return mix(source_vec3_start(row), source_vec3_start_b(row), start_config.bounds.x);
}

fn point_end(payload: vec4f) -> vec3f {
    let row = bitcast<u32>(payload.w);
    return mix(source_vec3_end(row), source_vec3_end_b(row), end_config.bounds.x);
}

fn atom_start(payload: vec4f) -> vec3f {
    let local = source_vec3_start(bitcast<u32>(payload.w));
    return (start_model.model_to_world * vec4f(local, 1.0)).xyz;
}

fn atom_end(payload: vec4f) -> vec3f {
    let local = source_vec3_end(bitcast<u32>(payload.w));
    return (end_model.model_to_world * vec4f(local, 1.0)).xyz;
}

fn rigid_start(payload: vec4f) -> vec3f {
    let base = bitcast<u32>(payload.w) * 8u;
    let source_a = bitcast<vec4f>(vec4u(start_source_a[base], start_source_a[base + 1u], start_source_a[base + 2u], start_source_a[base + 3u]));
    let orientation_a = bitcast<vec4f>(vec4u(start_source_a[base + 4u], start_source_a[base + 5u], start_source_a[base + 6u], start_source_a[base + 7u]));
    let source_b = bitcast<vec4f>(vec4u(start_source_b[base], start_source_b[base + 1u], start_source_b[base + 2u], start_source_b[base + 3u]));
    var orientation_b = bitcast<vec4f>(vec4u(start_source_b[base + 4u], start_source_b[base + 5u], start_source_b[base + 6u], start_source_b[base + 7u]));
    if dot(orientation_a, orientation_b) < 0.0 { orientation_b = -orientation_b; }
    let alpha = start_config.bounds.y * start_config.bounds.z;
    let translation_scale = mix(source_a, source_b, alpha);
    let orientation = normalize(mix(orientation_a, orientation_b, alpha));
    return translation_scale.xyz + rotate_vector(orientation, payload.xyz * translation_scale.w);
}

fn rigid_end(payload: vec4f) -> vec3f {
    let base = bitcast<u32>(payload.w) * 8u;
    let source_a = bitcast<vec4f>(vec4u(end_source_a[base], end_source_a[base + 1u], end_source_a[base + 2u], end_source_a[base + 3u]));
    let orientation_a = bitcast<vec4f>(vec4u(end_source_a[base + 4u], end_source_a[base + 5u], end_source_a[base + 6u], end_source_a[base + 7u]));
    let source_b = bitcast<vec4f>(vec4u(end_source_b[base], end_source_b[base + 1u], end_source_b[base + 2u], end_source_b[base + 3u]));
    var orientation_b = bitcast<vec4f>(vec4u(end_source_b[base + 4u], end_source_b[base + 5u], end_source_b[base + 6u], end_source_b[base + 7u]));
    if dot(orientation_a, orientation_b) < 0.0 { orientation_b = -orientation_b; }
    let alpha = end_config.bounds.y * end_config.bounds.z;
    let translation_scale = mix(source_a, source_b, alpha);
    let orientation = normalize(mix(orientation_a, orientation_b, alpha));
    return translation_scale.xyz + rotate_vector(orientation, payload.xyz * translation_scale.w);
}

fn linear_row(global_id: vec3u, groups: vec3u) -> u32 {
    return global_id.x + global_id.y * groups.x * 64u;
}

fn write_relation(row: u32, start: vec3f, end: vec3f) {
    let output = relation_resolvers[row].output.x;
    relation_output[output].start_width = vec4f(start, relation_output[output].start_width.w);
    relation_output[output].end_period = vec4f(end, relation_output[output].end_period.w);
}

@compute @workgroup_size(64)
fn resolve_world_world(@builtin(global_invocation_id) id: vec3u, @builtin(num_workgroups) groups: vec3u) {
    let row = linear_row(id, groups);
    if row < arrayLength(&relation_resolvers) { let value = relation_resolvers[row]; write_relation(row, world_start(value.start), world_end(value.end)); }
}

@compute @workgroup_size(64)
fn resolve_world_point(@builtin(global_invocation_id) id: vec3u, @builtin(num_workgroups) groups: vec3u) {
    let row = linear_row(id, groups);
    if row < arrayLength(&relation_resolvers) { let value = relation_resolvers[row]; write_relation(row, world_start(value.start), point_end(value.end)); }
}

@compute @workgroup_size(64)
fn resolve_world_atom(@builtin(global_invocation_id) id: vec3u, @builtin(num_workgroups) groups: vec3u) {
    let row = linear_row(id, groups);
    if row < arrayLength(&relation_resolvers) { let value = relation_resolvers[row]; write_relation(row, world_start(value.start), atom_end(value.end)); }
}

@compute @workgroup_size(64)
fn resolve_world_rigid(@builtin(global_invocation_id) id: vec3u, @builtin(num_workgroups) groups: vec3u) {
    let row = linear_row(id, groups);
    if row < arrayLength(&relation_resolvers) { let value = relation_resolvers[row]; write_relation(row, world_start(value.start), rigid_end(value.end)); }
}

@compute @workgroup_size(64)
fn resolve_point_world(@builtin(global_invocation_id) id: vec3u, @builtin(num_workgroups) groups: vec3u) {
    let row = linear_row(id, groups);
    if row < arrayLength(&relation_resolvers) { let value = relation_resolvers[row]; write_relation(row, point_start(value.start), world_end(value.end)); }
}

@compute @workgroup_size(64)
fn resolve_point_point(@builtin(global_invocation_id) id: vec3u, @builtin(num_workgroups) groups: vec3u) {
    let row = linear_row(id, groups);
    if row < arrayLength(&relation_resolvers) { let value = relation_resolvers[row]; write_relation(row, point_start(value.start), point_end(value.end)); }
}

@compute @workgroup_size(64)
fn resolve_point_atom(@builtin(global_invocation_id) id: vec3u, @builtin(num_workgroups) groups: vec3u) {
    let row = linear_row(id, groups);
    if row < arrayLength(&relation_resolvers) { let value = relation_resolvers[row]; write_relation(row, point_start(value.start), atom_end(value.end)); }
}

@compute @workgroup_size(64)
fn resolve_point_rigid(@builtin(global_invocation_id) id: vec3u, @builtin(num_workgroups) groups: vec3u) {
    let row = linear_row(id, groups);
    if row < arrayLength(&relation_resolvers) { let value = relation_resolvers[row]; write_relation(row, point_start(value.start), rigid_end(value.end)); }
}

@compute @workgroup_size(64)
fn resolve_atom_world(@builtin(global_invocation_id) id: vec3u, @builtin(num_workgroups) groups: vec3u) {
    let row = linear_row(id, groups);
    if row < arrayLength(&relation_resolvers) { let value = relation_resolvers[row]; write_relation(row, atom_start(value.start), world_end(value.end)); }
}

@compute @workgroup_size(64)
fn resolve_atom_point(@builtin(global_invocation_id) id: vec3u, @builtin(num_workgroups) groups: vec3u) {
    let row = linear_row(id, groups);
    if row < arrayLength(&relation_resolvers) { let value = relation_resolvers[row]; write_relation(row, atom_start(value.start), point_end(value.end)); }
}

@compute @workgroup_size(64)
fn resolve_atom_atom(@builtin(global_invocation_id) id: vec3u, @builtin(num_workgroups) groups: vec3u) {
    let row = linear_row(id, groups);
    if row < arrayLength(&relation_resolvers) { let value = relation_resolvers[row]; write_relation(row, atom_start(value.start), atom_end(value.end)); }
}

@compute @workgroup_size(64)
fn resolve_atom_rigid(@builtin(global_invocation_id) id: vec3u, @builtin(num_workgroups) groups: vec3u) {
    let row = linear_row(id, groups);
    if row < arrayLength(&relation_resolvers) { let value = relation_resolvers[row]; write_relation(row, atom_start(value.start), rigid_end(value.end)); }
}

@compute @workgroup_size(64)
fn resolve_rigid_world(@builtin(global_invocation_id) id: vec3u, @builtin(num_workgroups) groups: vec3u) {
    let row = linear_row(id, groups);
    if row < arrayLength(&relation_resolvers) { let value = relation_resolvers[row]; write_relation(row, rigid_start(value.start), world_end(value.end)); }
}

@compute @workgroup_size(64)
fn resolve_rigid_point(@builtin(global_invocation_id) id: vec3u, @builtin(num_workgroups) groups: vec3u) {
    let row = linear_row(id, groups);
    if row < arrayLength(&relation_resolvers) { let value = relation_resolvers[row]; write_relation(row, rigid_start(value.start), point_end(value.end)); }
}

@compute @workgroup_size(64)
fn resolve_rigid_atom(@builtin(global_invocation_id) id: vec3u, @builtin(num_workgroups) groups: vec3u) {
    let row = linear_row(id, groups);
    if row < arrayLength(&relation_resolvers) { let value = relation_resolvers[row]; write_relation(row, rigid_start(value.start), atom_end(value.end)); }
}

@compute @workgroup_size(64)
fn resolve_rigid_rigid(@builtin(global_invocation_id) id: vec3u, @builtin(num_workgroups) groups: vec3u) {
    let row = linear_row(id, groups);
    if row < arrayLength(&relation_resolvers) { let value = relation_resolvers[row]; write_relation(row, rigid_start(value.start), rigid_end(value.end)); }
}
