// Packed per-atom data and borrowed coordinate columns.
//
// Coordinates remain scalar arrays to preserve their packed 12-byte XYZ
// stride. Helpers operate on scalar record fields rather than whole records
// and avoid homogeneous matrix work when only affine XYZ is required.

//!include "include/records.wgsl"
//!include "include/visual/program_types.wgsl"

const ATOM_SOURCE_MASK: u32 = 0x1FFFFFFFu;
const ATOM_VISIBLE_FLAG: u32 = 0x00010000u;
const ATOM_SOFTNESS_SCALE: f32 = 8.0 / 255.0;

@group(2) @binding(0) var<storage, read> atoms: array<AtomRecord>;
@group(2) @binding(1) var<storage, read> coords: array<f32>;

struct ModelUniforms {
    model_to_world: mat4x4f,
    world_to_model: mat4x4f,
    previous_model_to_world: mat4x4f,
    pick_pages_a: vec4u,
    pick_pages_b: vec4u,
    pick_pages_c: vec4u,
}

@group(2) @binding(2) var<uniform> model: ModelUniforms;

@group(2) @binding(3) var<storage, read> bonds: array<BondRecord>;
@group(2) @binding(4) var<storage, read> visible_atoms: array<u32>;
@group(2) @binding(5) var<storage, read> visible_bonds: array<u32>;
@group(2) @binding(13) var<storage, read> previous_coords: array<f32>;

struct VisualCullCounts {
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

@group(2) @binding(14) var<uniform> visual_counts: VisualCullCounts;
@group(2) @binding(16) var<storage, read> visual_results: array<u32>;
@group(2) @binding(20) var<uniform> visual_config: VisualConfig;

const PICK_LOCAL_ROW_MASK: u32 = 0x0fffffffu;

fn pick_local_row(entity_id: u32) -> u32 {
    return entity_id & PICK_LOCAL_ROW_MASK;
}

fn model_pick_page(entity_id: u32) -> u32 {
    let kind = entity_id >> 28u;
    if kind < 4u {
        return model.pick_pages_a[kind];
    }
    if kind < 8u {
        return model.pick_pages_b[kind - 4u];
    }
    return model.pick_pages_c[kind - 8u];
}

struct AtomMotionPositions {
    current: vec3f,
    previous: vec3f,
}

/// Transforms an affine point without computing the unused homogeneous W.
fn atom_transform_point(
    transform: mat4x4f,
    position: vec3f,
) -> vec3f {
    return transform[0].xyz * position.x
        + transform[1].xyz * position.y
        + transform[2].xyz * position.z
        + transform[3].xyz;
}

/// Returns the original coordinate-row index encoded in an entity id.
fn atom_source_index(entity_id: u32) -> u32 {
    return entity_id & ATOM_SOURCE_MASK;
}

fn atom_visual_offset(entity_id: u32) -> vec3f {
    if visual_counts.visual_enabled == 0u {
        return vec3f(0.0);
    }
    return visual_result_offset(atom_source_index(entity_id));
}

/// Returns the scalar-array offset of an atom's packed XYZ coordinate.
fn atom_coordinate_base(entity_id: u32) -> u32 {
    return atom_source_index(entity_id) * 3u;
}

/// Loads the current packed local-space XYZ coordinate.
fn atom_local_position(base: u32) -> vec3f {
    return vec3f(
        coords[base],
        coords[base + 1u],
        coords[base + 2u],
    );
}

/// Loads the previous packed local-space XYZ coordinate.
fn previous_atom_local_position(base: u32) -> vec3f {
    return vec3f(
        previous_coords[base],
        previous_coords[base + 1u],
        previous_coords[base + 2u],
    );
}

/// Returns the current world-space atom position.
fn atom_position(entity_id: u32) -> vec3f {
    return atom_transform_point(
        model.model_to_world,
        atom_local_position(atom_coordinate_base(entity_id))
            + atom_visual_offset(entity_id),
    );
}

/// Returns the previous world-space atom position.
fn previous_atom_position(entity_id: u32) -> vec3f {
    return atom_transform_point(
        model.previous_model_to_world,
        previous_atom_local_position(atom_coordinate_base(entity_id))
            + atom_visual_offset(entity_id),
    );
}

/// Resolves current and previous positions while decoding the source row once.
fn atom_motion_positions(entity_id: u32) -> AtomMotionPositions {
    let base =
        atom_coordinate_base(entity_id);

    return AtomMotionPositions(
        atom_transform_point(
            model.model_to_world,
            atom_local_position(base),
        ),
        atom_transform_point(
            model.previous_model_to_world,
            previous_atom_local_position(base),
        ),
    );
}

/// Decodes packed 8-bit RGBA directly to normalized floats.
fn atom_color(packed: u32) -> vec4f {
    return unpack4x8unorm(packed);
}

/// Resolves the optional entity result while retaining the byte-identical
/// built-in path when no visual style is attached.
fn atom_visual_color(entity_id: u32, packed: u32) -> vec4f {
    var fallback = atom_color(packed);
    if visual_counts.visual_enabled == 0u {
        return fallback;
    }
    fallback = visual_uniform_base_color(fallback);
    return unpack4x8unorm(visual_result_word(
        atom_source_index(entity_id),
        VISUAL_RESULT_COLOR,
        pack4x8unorm(fallback),
    ));
}

fn atom_visual_response(entity_id: u32) -> vec4f {
    if visual_counts.visual_enabled == 0u {
        return vec4f(0.34, 0.5, 0.0, 0.0);
    }
    return unpack4x8unorm(visual_result_word(
        atom_source_index(entity_id),
        VISUAL_RESULT_RESPONSE,
        pack4x8unorm(vec4f(
            visual_config.material.y,
            visual_config.material.z,
            visual_config.material.w,
            0.0,
        )),
    ));
}

fn atom_visual_geometry(entity_id: u32) -> vec4f {
    if visual_counts.visual_enabled == 0u {
        return vec4f(0.0, 0.0, 0.25, 0.25);
    }
    return unpack4x8unorm(visual_result_word(
        atom_source_index(entity_id),
        VISUAL_RESULT_GEOMETRY,
        pack4x8unorm(visual_config.uniform_geometry),
    ));
}

fn atom_visual_emission(entity_id: u32) -> vec3f {
    if visual_counts.visual_enabled == 0u {
        return vec3f(0.0);
    }
    return unpack4x8unorm(visual_result_word(
        atom_source_index(entity_id),
        VISUAL_RESULT_EMISSION,
        pack4x8unorm(visual_config.uniform_emission / 64.0),
    )).rgb * 64.0;
}

/// Tests the packed atom visibility flag.
fn atom_visible(element_flags: u32) -> bool {
    return (
        element_flags &
        ATOM_VISIBLE_FLAG
    ) != 0u;
}

/// Decodes the representation-local analytic edge softness in pixels.
fn atom_softness_pixels(semantic: u32) -> f32 {
    return f32(semantic >> 24u) *
        ATOM_SOFTNESS_SCALE;
}
