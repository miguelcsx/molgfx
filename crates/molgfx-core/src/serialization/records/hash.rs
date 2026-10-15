//! Stable content fingerprints for caller-owned columns.

const FNV_OFFSET: u64 = 14_695_981_039_346_656_037;
const FNV_PRIME: u64 = 1_099_511_628_211;

pub(crate) fn value_hash(values: &[f32]) -> u64 {
    let mut hash = FNV_OFFSET;
    for value in values {
        for byte in value.to_bits().to_le_bytes() {
            hash_byte(&mut hash, byte);
        }
    }
    hash
}

pub(crate) fn label_hash(values: &[u32]) -> u64 {
    let mut hash = FNV_OFFSET;
    for value in values {
        for byte in value.to_le_bytes() {
            hash_byte(&mut hash, byte);
        }
    }
    hash
}

pub(crate) fn mesh_hash(mesh: &crate::Mesh) -> u64 {
    let mut hash = FNV_OFFSET;
    for vertex in mesh.vertices() {
        for value in vertex
            .position
            .to_array()
            .into_iter()
            .chain(vertex.normal.to_array())
        {
            for byte in value.to_bits().to_le_bytes() {
                hash_byte(&mut hash, byte);
            }
        }
        for byte in [
            vertex.color.r,
            vertex.color.g,
            vertex.color.b,
            vertex.color.a,
        ] {
            hash_byte(&mut hash, byte);
        }
    }
    for index in mesh.indices() {
        for byte in index.to_le_bytes() {
            hash_byte(&mut hash, byte);
        }
    }
    hash
}

fn hash_byte(hash: &mut u64, byte: u8) {
    *hash ^= u64::from(byte);
    *hash = hash.wrapping_mul(FNV_PRIME);
}
