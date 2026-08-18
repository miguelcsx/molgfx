// GPU visibility compaction, visibility caching, and indirect argument authorship.

//!include "include/records.wgsl"
//!include "include/frame_record.wgsl"

const ENTITY_ID_MASK: u32 = 0x1FFFFFFFu;
const VISIBLE_FLAG: u32 = 0x00010000u;
const MIN_DISTANCE: f32 = 1e-4;

struct DrawArgs {
    vertex_count: u32,
    instance_count: atomic<u32>,
    first_vertex: u32,
    first_instance: u32,
}

struct CullCounts {
    atoms: u32,
    bonds: u32,
    padding_a: u32,
    padding_b: u32,
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
@group(0) @binding(10) var<storage, read_write> atom_visibility: array<u32>;

var<workgroup> visible_count: atomic<u32>;
var<workgroup> output_base: u32;

/// Resolves the streamed position for an atom.
fn resolved_position(entity_id: u32) -> vec3f {
    let base = (entity_id & ENTITY_ID_MASK) * 3u;
    return vec3f(coordinates[base], coordinates[base + 1u], coordinates[base + 2u]);
}

/// Performs the expensive geometric visibility test once per atom.
fn is_visible(index: u32) -> bool {
    let radius = input_atoms[index].radius;

    if (input_atoms[index].element_flags & VISIBLE_FLAG) == 0u || radius <= 0.0 {
        return false;
    }

    let world = model_to_world * vec4f(
        resolved_position(input_atoms[index].entity_id),
        1.0,
    );

    let clip = frame.view_proj * world;
    let w = clip.w;

    if w <= 0.0 || clip.z < 0.0 || clip.z > w {
        return false;
    }

    let distance = max(-(frame.view * world).z, MIN_DISTANCE);
    let projected_radius = radius * w / distance;
    let extent = abs(vec2f(frame.proj[0][0], frame.proj[1][1])) * projected_radius;

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

@compute @workgroup_size(64)
fn cull_atoms(
    @builtin(global_invocation_id) id: vec3u,
    @builtin(local_invocation_index) local_index: u32,
) {
    let index = id.x;
    var keep = false;

    if index < counts.atoms {
        keep = is_visible(index);
        atom_visibility[index] = select(0u, 1u, keep);
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
) {
    let index = id.x;
    var keep = false;

    if index < counts.bonds {
        let atom_a = input_bonds[index].atom_a;

        if atom_visibility[atom_a] != 0u {
            keep = atom_visibility[input_bonds[index].atom_b] != 0u;
        }
    }

    let compact = compact_slot(local_index, keep, false);

    if keep {
        output_bonds[compact] = index;
    }
}
