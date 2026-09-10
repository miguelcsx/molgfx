// Generational sparse scalar-brick lookup.
//
// The hash table is proportional to the resident working set. Logical origins
// remain split into low/high words so catalog coordinates never truncate even
// though the current raster volume transform addresses a u32 voxel window.

override VOLUME_SPARSE_BRICKS: bool = false;

struct SparseBrickPage {
    origin_lo_mip: vec4u,
    origin_hi_valid: vec4u,
    slot_shape: vec4u,
    generation_kind: vec4u,
}

struct SparseBrickUniforms {
    table: vec4u,
    stored_shape: vec4u,
    interior_shape: vec4u,
}

@group(2) @binding(3) var<storage, read> sparse_brick_pages: array<SparseBrickPage>;
@group(2) @binding(4) var<uniform> sparse_bricks: SparseBrickUniforms;

fn sparse_hash(origin_lo: vec3u, origin_hi: vec3u, mip: u32) -> u32 {
    var hash = 2166136261u;
    hash = (hash ^ origin_lo.x) * 16777619u;
    hash = (hash ^ origin_hi.x) * 16777619u;
    hash = (hash ^ origin_lo.y) * 16777619u;
    hash = (hash ^ origin_hi.y) * 16777619u;
    hash = (hash ^ origin_lo.z) * 16777619u;
    hash = (hash ^ origin_hi.z) * 16777619u;
    return (hash ^ mip) * 16777619u;
}

fn sparse_scalar_texel(global_texel: vec3u) -> f32 {
    let mip = sparse_bricks.table.y;
    let scale = 1u << min(mip, 31u);
    let mip_texel = global_texel / vec3u(scale);
    let interior = max(sparse_bricks.interior_shape.xyz, vec3u(1u));
    let origin = (mip_texel / interior) * interior;
    let high = vec3u(0u);
    let mask = sparse_bricks.table.x;
    var index = sparse_hash(origin, high, mip) & mask;

    for (var probe = 0u; probe <= mask; probe++) {
        let page = sparse_brick_pages[index];
        if page.origin_hi_valid.w == 0u {
            return 0.0;
        }
        if all(page.origin_lo_mip.xyz == origin) &&
            all(page.origin_hi_valid.xyz == high) &&
            page.origin_lo_mip.w == mip {
            let halo = page.generation_kind.z;
            let local = mip_texel - origin + vec3u(halo);
            let atlas_z = page.slot_shape.x * sparse_bricks.stored_shape.z + local.z;
            return textureLoad(
                density_texture,
                vec3i(vec3u(local.x, local.y, atlas_z)),
                0,
            ).x;
        }
        index = (index + 1u) & mask;
    }
    return 0.0;
}

fn volume_scalar_texel(coordinate: vec3i) -> f32 {
    if VOLUME_SPARSE_BRICKS {
        return sparse_scalar_texel(vec3u(max(coordinate, vec3i(0))));
    }
    return textureLoad(density_texture, coordinate, 0).x;
}
