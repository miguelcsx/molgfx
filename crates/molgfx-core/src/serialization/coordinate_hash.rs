//! Stable fingerprint of caller-owned structure identity and coordinates.

pub(super) fn coordinate_hash(placed: &crate::PlacedStructure) -> u64 {
    let mut hash = 14_695_981_039_346_656_037u64;
    let update = |hash: &mut u64, byte: u8| {
        *hash ^= u64::from(byte);
        *hash = hash.wrapping_mul(1_099_511_628_211);
    };
    for byte in placed.source.identity().to_le_bytes() {
        update(&mut hash, byte);
    }
    if let Some(id) = placed
        .source
        .molframe()
        .and_then(|structure| structure.metadata().id.as_deref())
    {
        for byte in id.as_bytes() {
            update(&mut hash, *byte);
        }
    }
    for coordinate in placed.atoms.coords().slice() {
        for component in coordinate {
            for byte in component.to_bits().to_le_bytes() {
                update(&mut hash, byte);
            }
        }
    }
    hash
}
