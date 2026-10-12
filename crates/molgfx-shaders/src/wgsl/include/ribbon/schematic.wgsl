// Five immutable lanes per guide: neighboring CA rows, strand flag and oxygen row.
const RIBBON_SCHEMATIC_ANCHOR: u32 = 14u;

fn ribbon_schematic_coordinate(stencil: u32, previous: bool, reference: bool) -> vec3f {
    let rows = bitcast<vec3u>(vec3f(
        ribbon_radius_sources[stencil],
        ribbon_radius_sources[stencil + 1u],
        ribbon_radius_sources[stencil + 2u],
    ));
    let anchor_kind = bitcast<u32>(ribbon_radius_sources[stencil + 3u]);
    if anchor_kind == 2u {
        let state = select(select(0u, 1u, previous), 2u, reference);
        return ribbon_fit_vector(rows.x + state * rows.z);
    }
    if anchor_kind == 3u {
        let state = select(select(0u, 8u, previous), 16u, reference);
        return ribbon_fit_vector(rows.x + state)
            + ribbon_fit_vector(rows.x + state + 4u) * bitcast<f32>(rows.z);
    }
    var center = ribbon_coordinate(rows.y, previous);
    if reference { center = ribbon_base_coordinate(rows.y); }
    if bitcast<u32>(ribbon_radius_sources[stencil + 3u]) == 0u { return center; }
    var left = ribbon_coordinate(rows.x, previous);
    var right = ribbon_coordinate(rows.z, previous);
    if reference {
        left = ribbon_base_coordinate(rows.x);
        right = ribbon_base_coordinate(rows.z);
    }
    return left * 0.25 + center * 0.5 + right * 0.25;
}

fn ribbon_schematic_oxygen(stencil: u32, previous: bool, reference: bool) -> vec3f {
    let ca = bitcast<u32>(ribbon_radius_sources[stencil + 1u]);
    let oxygen = bitcast<u32>(ribbon_radius_sources[stencil + 4u]);
    if oxygen == 0xffffffffu { return vec3f(0.0); }
    if reference { return ribbon_base_coordinate(oxygen) - ribbon_base_coordinate(ca); }
    return ribbon_coordinate(oxygen, previous) - ribbon_coordinate(ca, previous);
}

fn ribbon_anchor_coordinate(row: u32, mode: u32, previous: bool, reference: bool) -> vec3f {
    if mode == RIBBON_SCHEMATIC_ANCHOR { return ribbon_schematic_coordinate(row, previous, reference); }
    if reference { return ribbon_base_coordinate(row); }
    return ribbon_coordinate(row, previous);
}
