// Binary records shared by geometry and compute culling shaders.

struct AtomRecord {
    radius: f32,
    color: u32,
    element_flags: u32,
    entity_id: u32,
    semantic: u32,
}

struct BondRecord {
    atom_a: u32,
    atom_b: u32,
    radius: f32,
    entity_id: u32,
}
