// GPU visibility compaction, visibility caching, and indirect argument authorship.

//!include "include/records.wgsl"
//!include "include/frame_record.wgsl"
//!include "include/visual/program_types.wgsl"

const ENTITY_ID_MASK: u32 = 0x1FFFFFFFu;
const VISIBLE_FLAG: u32 = 0x00010000u;
const TILE_SIZE: u32 = 8u;
const MAX_LOD_RADIUS_PIXELS: f32 = 8.0;
const TILE_BITS: u32 = 18u;
const TILE_MASK: u32 = (1u << TILE_BITS) - 1u;
const DEPTH_LEVELS: f32 = 16383.0;
const POINT_INDEX_BITS_SMALL: u32 = 20u;
const POINT_INDEX_BITS_MEDIUM: u32 = 22u;
const POINT_INDEX_BITS_LARGE: u32 = 24u;

struct DrawArgs {
    vertex_count: u32,
    instance_count: atomic<u32>,
    first_vertex: u32,
    first_instance: u32,
}

struct CullCounts {
    atoms: u32,
    bonds: u32,
    lod_enabled: u32,
    padding_b: u32,
    bond_break_length: f32,
    visual_enabled: u32,
    atom_bvh_nodes: u32,
    atom_bvh_indices: u32,
    bond_bvh_nodes: u32,
    bond_bvh_indices: u32,
}

struct AtomProjection {
    visible: u32,
    tile: u32,
    depth: u32,
    lod: u32,
}

@group(0) @binding(0) var<storage, read> input_atoms: array<AtomRecord>;
@group(0) @binding(1) var<storage, read> input_bonds: array<BondRecord>;
@group(0) @binding(2) var<storage, read_write> output_atoms: array<u32>;
@group(0) @binding(3) var<storage, read_write> output_bonds: array<u32>;
@group(0) @binding(4) var<storage, read_write> atom_args: DrawArgs;
@group(0) @binding(5) var<storage, read_write> bond_args: DrawArgs;
@group(0) @binding(6) var<uniform> counts: CullCounts;
@group(0) @binding(7) var<uniform> frame: FrameUniforms;
@group(0) @binding(8) var<uniform> model_to_world: mat4x4f;
@group(0) @binding(9) var<storage, read> coordinates: array<f32>;
@group(0) @binding(10) var<storage, read_write> tile_depth: array<atomic<u32>>;
@group(0) @binding(14) var<storage, read_write> visual_results: array<u32>;
@group(0) @binding(15) var<uniform> visual_config: VisualConfig;

var<workgroup> visible_count: atomic<u32>;
var<workgroup> output_base: u32;

/// Resolves the streamed position for an atom.
fn resolved_position(entity_id: u32) -> vec3f {
    let base = (entity_id & ENTITY_ID_MASK) * 3u;
    let position = vec3f(coordinates[base], coordinates[base + 1u], coordinates[base + 2u]);
    if counts.visual_enabled == 0u {
        return position;
    }
    return position + visual_result_offset(entity_id & ENTITY_ID_MASK);
}

/// Performs the expensive geometric visibility test once per atom.
fn project_atom(index: u32) -> AtomProjection {
    let atom = input_atoms[index];
    var radius = atom.radius;

    if (atom.element_flags & VISIBLE_FLAG) == 0u || radius <= 0.0 {
        return AtomProjection(0u, 0u, 0u, 0u);
    }
    let geometry = visual_result_word(
        atom.entity_id & ENTITY_ID_MASK,
        VISUAL_RESULT_GEOMETRY,
        pack4x8unorm(visual_config.uniform_geometry),
    );
    if counts.visual_enabled != 0u && (geometry & 0xffu) == 0u {
        return AtomProjection(0u, 0u, 0u, 0u);
    }
    if counts.visual_enabled != 0u {
        radius *= f32((geometry >> 16u) & 0xffu) / 64.0;
    }

    let local = resolved_position(atom.entity_id);

    // Model transforms are affine. Keep the known homogeneous component out
    // of both matrix products and construct it only for clip projection.
    let world =
        model_to_world[0].xyz * local.x
        + model_to_world[1].xyz * local.y
        + model_to_world[2].xyz * local.z
        + model_to_world[3].xyz;

    let clip = frame.view_proj * vec4f(world, 1.0);
    let w = clip.w;

    if w <= 0.0 || clip.z < 0.0 || clip.z > w {
        return AtomProjection(0u, 0u, 0u, 0u);
    }

    if counts.lod_enabled == 2u {
        if !all(abs(clip.xy) <= vec2f(w)) {
            return AtomProjection(0u, 0u, 0u, 0u);
        }
        let ndc = clip.xy / w;
        let pixel = clamp(
            (ndc * vec2f(0.5, -0.5) + vec2f(0.5)) * frame.viewport.xy,
            vec2f(0.0),
            max(frame.viewport.xy - vec2f(1.0), vec2f(0.0)),
        );
        let tile_width = u32(ceil(frame.viewport.x / f32(TILE_SIZE)));
        let tile_xy = vec2u(pixel) / vec2u(TILE_SIZE);
        let tile = tile_xy.y * tile_width + tile_xy.x;
        let tile_count = tile_width * u32(ceil(frame.viewport.y / f32(TILE_SIZE)));
        let depth = u32(round(clamp(clip.z / w, 0.0, 1.0) * DEPTH_LEVELS));
        return AtomProjection(1u, tile, depth, u32(tile_count < TILE_MASK));
    }

    let extent = frame.projection_kind.yz * radius;

    if !all(abs(clip.xy) <= vec2f(w, w) + extent) {
        return AtomProjection(0u, 0u, 0u, 0u);
    }

    let ndc = clip.xy / w;
    let pixel = clamp(
        (ndc * vec2f(0.5, -0.5) + vec2f(0.5)) * frame.viewport.xy,
        vec2f(0.0),
        max(frame.viewport.xy - vec2f(1.0), vec2f(0.0)),
    );
    let tile_width = u32(ceil(frame.viewport.x / f32(TILE_SIZE)));
    let tile_xy = vec2u(pixel) / vec2u(TILE_SIZE);
    let tile = tile_xy.y * tile_width + tile_xy.x;
    let radius_pixels = max(
        extent.x / w * frame.viewport.x * 0.5,
        extent.y / w * frame.viewport.y * 0.5,
    );
    let tile_count = tile_width * u32(ceil(frame.viewport.y / f32(TILE_SIZE)));
    let lod = u32(radius_pixels <= MAX_LOD_RADIUS_PIXELS && tile_count < TILE_MASK);
    let depth = u32(round(clamp(clip.z / w, 0.0, 1.0) * DEPTH_LEVELS));
    return AtomProjection(1u, tile, depth, lod);
}

/// Reports whether a bond is stretched past the covalent break length.
///
/// The endpoints are compared in model space, where the break length is
/// expressed; a non-positive length disables breaking so static scenes draw
/// every bond exactly as before.
fn bond_over_break_length(bond: BondRecord) -> bool {
    if counts.bond_break_length <= 0.0 {
        return false;
    }
    let position_a = resolved_position(input_atoms[bond.atom_a].entity_id);
    let position_b = resolved_position(input_atoms[bond.atom_b].entity_id);
    let delta = position_a - position_b;
    let break_length = counts.bond_break_length;
    return dot(delta, delta) > break_length * break_length;
}

fn is_visible(index: u32) -> bool {
    let atom = input_atoms[index];
    var radius = atom.radius;
    if (atom.element_flags & VISIBLE_FLAG) == 0u || radius <= 0.0 {
        return false;
    }
    let geometry = visual_result_word(
        atom.entity_id & ENTITY_ID_MASK,
        VISUAL_RESULT_GEOMETRY,
        pack4x8unorm(visual_config.uniform_geometry),
    );
    if counts.visual_enabled != 0u && (geometry & 0xffu) == 0u {
        return false;
    }
    if counts.visual_enabled != 0u {
        radius *= f32((geometry >> 16u) & 0xffu) / 64.0;
    }
    let local = resolved_position(atom.entity_id);
    let world =
        model_to_world[0].xyz * local.x
        + model_to_world[1].xyz * local.y
        + model_to_world[2].xyz * local.z
        + model_to_world[3].xyz;
    let clip = frame.view_proj * vec4f(world, 1.0);
    let w = clip.w;
    if w <= 0.0 || clip.z < 0.0 || clip.z > w {
        return false;
    }
    let extent = frame.projection_kind.yz * radius;
    return all(abs(clip.xy) <= vec2f(w, w) + extent);
}

/// Reserves one global output range per workgroup.
fn compact_slot(
    local_index: u32,
    keep: bool,
    atoms: bool,
) -> u32 {
    var local = 0u;

    if keep {
        local = atomicAdd(&visible_count, 1u);
    }

    let count = workgroupUniformLoad(&visible_count);

    if local_index == 0u && count != 0u {
        if atoms {
            output_base = atomicAdd(&atom_args.instance_count, count);
        } else {
            output_base = atomicAdd(&bond_args.instance_count, count);
        }
    }

    return workgroupUniformLoad(&output_base) + local;
}

@compute @workgroup_size(1)
fn reset_cull() {
    atomicStore(&atom_args.instance_count, 0u);
    atomicStore(&bond_args.instance_count, 0u);
}

fn point_index_bits() -> u32 {
    if counts.atoms <= (1u << POINT_INDEX_BITS_SMALL) {
        return POINT_INDEX_BITS_SMALL;
    }
    if counts.atoms <= (1u << POINT_INDEX_BITS_MEDIUM) {
        return POINT_INDEX_BITS_MEDIUM;
    }
    return POINT_INDEX_BITS_LARGE;
}

@compute @workgroup_size(64)
fn reset_tiles(
    @builtin(global_invocation_id) id: vec3u,
    @builtin(num_workgroups) groups: vec3u,
) {
    let index = id.x + id.y * groups.x * 64u;
    let tile_count = u32(ceil(frame.viewport.x / f32(TILE_SIZE)))
        * u32(ceil(frame.viewport.y / f32(TILE_SIZE)));
    if index < tile_count {
        atomicStore(&tile_depth[index], 0u);
    }
}

@compute @workgroup_size(64)
fn bin_atoms(
    @builtin(global_invocation_id) id: vec3u,
    @builtin(num_workgroups) groups: vec3u,
) {
    let linear_index = id.x + id.y * groups.x * 64u;
    var index = linear_index;
    let tile_count = u32(ceil(frame.viewport.x / f32(TILE_SIZE)))
        * u32(ceil(frame.viewport.y / f32(TILE_SIZE)));
    if counts.lod_enabled == 2u && tile_count < TILE_MASK {
        let candidates = (counts.atoms + counts.padding_b - 1u) / counts.padding_b;
        var hash = linear_index;
        hash ^= hash >> 16u;
        hash *= 0x7feb352du;
        hash ^= hash >> 15u;
        let sampled = linear_index + (hash % counts.padding_b) * candidates;
        index = select(linear_index, sampled, sampled < counts.atoms);
    }
    if index >= counts.atoms {
        return;
    }
    let projected = project_atom(index);
    if projected.visible == 0u {
        output_atoms[counts.atoms + index] = 0u;
        return;
    }
    if projected.lod == 0u {
        output_atoms[counts.atoms + index] = 0xffffffffu;
        return;
    }
    if counts.lod_enabled == 2u {
        let index_bits = point_index_bits();
        let index_mask = (1u << index_bits) - 1u;
        let depth_levels = f32((1u << (32u - index_bits)) - 2u);
        let depth = 1u + u32(round(
            f32(projected.depth) / DEPTH_LEVELS * depth_levels
        ));
        let candidate = (depth << index_bits) | (index_mask - index);
        atomicMax(&tile_depth[projected.tile], candidate);
        return;
    }
    let depth = projected.depth;
    output_atoms[counts.atoms + index] = (depth << TILE_BITS) | (projected.tile + 1u);
    atomicMax(&tile_depth[projected.tile], depth);
}

@compute @workgroup_size(64)
fn compact_tiles(
    @builtin(global_invocation_id) id: vec3u,
    @builtin(local_invocation_index) local_index: u32,
    @builtin(num_workgroups) groups: vec3u,
) {
    let index = id.x + id.y * groups.x * 64u;
    let tile_count = u32(ceil(frame.viewport.x / f32(TILE_SIZE)))
        * u32(ceil(frame.viewport.y / f32(TILE_SIZE)));
    var winner = 0u;
    if index < tile_count {
        winner = atomicLoad(&tile_depth[index]);
    }
    let keep = winner != 0u;
    let compact = compact_slot(local_index, keep, true);
    if keep {
        let index_bits = point_index_bits();
        let index_mask = (1u << index_bits) - 1u;
        output_atoms[compact] = index_mask - (winner & index_mask);
    }
}

@compute @workgroup_size(64)
fn cull_atoms(
    @builtin(global_invocation_id) id: vec3u,
    @builtin(local_invocation_index) local_index: u32,
    @builtin(num_workgroups) groups: vec3u,
) {
    let index = id.x + id.y * groups.x * 64u;
    var keep = false;

    if index < counts.atoms {
        if counts.lod_enabled == 0u {
            keep = is_visible(index);
            if counts.bonds != 0u {
                output_atoms[counts.atoms + index] = select(0u, 1u, keep);
            }
        } else {
            let cached = output_atoms[counts.atoms + index];
            keep = cached != 0u;
            if keep && cached != 0xffffffffu {
                let tile = (cached & TILE_MASK) - 1u;
                keep = atomicLoad(&tile_depth[tile]) == cached >> TILE_BITS;
            }
            output_atoms[counts.atoms + index] = select(0u, 1u, keep);
        }
    }

    let compact = compact_slot(local_index, keep, true);

    if keep {
        output_atoms[compact] = index;
    }
}

@compute @workgroup_size(64)
fn cull_bonds(
    @builtin(global_invocation_id) id: vec3u,
    @builtin(local_invocation_index) local_index: u32,
    @builtin(num_workgroups) groups: vec3u,
) {
    let index = id.x + id.y * groups.x * 64u;
    var keep = false;

    if index < counts.bonds {
        let bond = input_bonds[index];
        let atom_a = bond.atom_a;

        if output_atoms[counts.atoms + atom_a] != 0u {
            keep = output_atoms[counts.atoms + bond.atom_b] != 0u;
        }
        if keep && bond_over_break_length(bond) {
            keep = false;
        }
    }

    let compact = compact_slot(local_index, keep, false);

    if keep {
        output_bonds[compact] = index;
    }
}

/// Authors a stable wire-bond stream without materializing the otherwise-unused
/// visible-atom stream. Invisible entries retain their input slot as a sentinel
/// so workgroup scheduling cannot change primitive order or shared-endpoint
/// depth ownership between frames.
@compute @workgroup_size(64)
fn cull_bonds_direct(
    @builtin(global_invocation_id) id: vec3u,
    @builtin(num_workgroups) groups: vec3u,
) {
    let index = id.x + id.y * groups.x * 64u;
    if index < counts.bonds {
        let bond = input_bonds[index];
        let keep = is_visible(bond.atom_a)
            && is_visible(bond.atom_b)
            && !bond_over_break_length(bond);
        output_bonds[index] = select(0xffffffffu, index, keep);
    }
    if index == 0u {
        atomicStore(&bond_args.instance_count, counts.bonds);
    }
}
