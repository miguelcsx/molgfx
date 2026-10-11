// O(selected atoms) per sweep of residue centers and enclosure radii.
// The recipe contains source rows and radii, never a coordinate copy.
const RESIDUE_CENTER_FLAG: u32 = 0x00200000u;

fn bead_geometry(atom: AtomRecord, previous: bool) -> vec4f {
    let row = atom.entity_id & 0x0fffffffu;
    if (atom.element_flags & RESIDUE_CENTER_FLAG) == 0u {
        return vec4f(bead_coordinate(row, previous), atom.radius);
    }
    let compact = bead_word(1u + row);
    let header = 1u + bead_word(0u) + compact * 3u;
    let start = bead_word(header);
    let count = bead_word(header + 1u);
    let scale = bitcast<f32>(bead_word(header + 2u));
    var center = vec3f(0.0);
    for (var member = 0u; member < count; member++) {
        center += bead_coordinate(bead_word(start + member * 2u), previous);
    }
    center /= f32(max(count, 1u));
    var radius = 0.0;
    for (var member = 0u; member < count; member++) {
        let word = start + member * 2u;
        let point = bead_coordinate(bead_word(word), previous);
        radius = max(radius, distance(center, point) + bitcast<f32>(bead_word(word + 1u)));
    }
    return vec4f(center, radius * scale);
}
