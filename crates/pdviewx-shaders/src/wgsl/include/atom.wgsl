// Packed per-atom data and borrowed coordinate columns.
//
// Coordinates remain scalar arrays to preserve their packed 12-byte XYZ
// stride. Helpers operate on scalar record fields rather than whole records
// and avoid homogeneous matrix work when only affine XYZ is required.

//!include "include/records.wgsl"

const ATOM_SOURCE_MASK: u32 = 0x1FFFFFFFu;
const ATOM_VISIBLE_FLAG: u32 = 0x00010000u;
const ATOM_SOFTNESS_SCALE: f32 = 8.0 / 255.0;

@group(2) @binding(0) var<storage, read> atoms: array<AtomRecord>;
@group(2) @binding(1) var<storage, read> coords: array<f32>;

struct ModelUniforms {
    model_to_world: mat4x4f,
    world_to_model: mat4x4f,
    previous_model_to_world: mat4x4f,
    structure_id: u32,
    padding_a: u32,
    padding_b: u32,
    padding_c: u32,
}

@group(2) @binding(2) var<uniform> model: ModelUniforms;

@group(2) @binding(3) var<storage, read> bonds: array<BondRecord>;
@group(2) @binding(4) var<storage, read> visible_atoms: array<u32>;
@group(2) @binding(5) var<storage, read> visible_bonds: array<u32>;
@group(2) @binding(13) var<storage, read> previous_coords: array<f32>;

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
        atom_local_position(
            atom_coordinate_base(entity_id),
        ),
    );
}

/// Returns the previous world-space atom position.
fn previous_atom_position(entity_id: u32) -> vec3f {
    return atom_transform_point(
        model.previous_model_to_world,
        previous_atom_local_position(
            atom_coordinate_base(entity_id),
        ),
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
