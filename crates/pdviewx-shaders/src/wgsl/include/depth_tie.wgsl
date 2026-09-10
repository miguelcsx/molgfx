// Deterministic ownership for analytically coincident opaque fragments.
//
// GPU culling deliberately compacts visibility in parallel, so draw order is
// not stable across submissions. Positive normalized depth is monotonic in its
// IEEE-754 bits; a two-bit entity rank resolves exact depth ties while moving a
// surface by at most three representable depth values.

fn stable_entity_depth(depth: f32, entity_id: u32) -> f32 {
    let bounded = clamp(depth, 0.0, 1.0);
    let bits = bitcast<u32>(bounded);
    let one = bitcast<u32>(1.0);
    return bitcast<f32>(min(bits + (entity_id & 0x3u), one));
}
