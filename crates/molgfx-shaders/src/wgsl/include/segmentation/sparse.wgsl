// Generational sparse categorical-brick lookup. Labels remain integer and
// nearest-sampled; halo texels preserve boundary normals across resident pages.

override SEGMENTATION_SPARSE_BRICKS: bool = false;

struct SparseSegmentPage {
    origin_lo_mip: vec4u,
    origin_hi_valid: vec4u,
    slot_shape: vec4u,
    generation_kind: vec4u,
}

struct SparseSegmentUniforms {
    table: vec4u,
    stored_shape: vec4u,
    interior_shape: vec4u,
}

@group(2) @binding(3) var<storage, read> sparse_segment_pages: array<SparseSegmentPage>;
@group(2) @binding(4) var<uniform> sparse_segments: SparseSegmentUniforms;

fn sparse_segment_hash(origin: vec3u, high: vec3u, mip: u32) -> u32 {
    var hash = 2166136261u;
    hash = (hash ^ origin.x) * 16777619u;
    hash = (hash ^ high.x) * 16777619u;
    hash = (hash ^ origin.y) * 16777619u;
    hash = (hash ^ high.y) * 16777619u;
    hash = (hash ^ origin.z) * 16777619u;
    hash = (hash ^ high.z) * 16777619u;
    return (hash ^ mip) * 16777619u;
}

fn sparse_label_texel(global_texel: vec3u) -> u32 {
    let mip = sparse_segments.table.y;
    let scale = 1u << min(mip, 31u);
    let mip_texel = global_texel / vec3u(scale);
    let interior = max(sparse_segments.interior_shape.xyz, vec3u(1u));
    let origin = (mip_texel / interior) * interior;
    let high = vec3u(0u);
    let mask = sparse_segments.table.x;
    var index = sparse_segment_hash(origin, high, mip) & mask;

    for (var probe = 0u; probe <= mask; probe++) {
        let page = sparse_segment_pages[index];
        if page.origin_hi_valid.w == 0u {
            return 0u;
        }
        if all(page.origin_lo_mip.xyz == origin) &&
            all(page.origin_hi_valid.xyz == high) &&
            page.origin_lo_mip.w == mip {
            let local = mip_texel - origin + vec3u(page.generation_kind.z);
            let atlas_z = page.slot_shape.x * sparse_segments.stored_shape.z + local.z;
            return textureLoad(
                label_texture,
                vec3i(vec3u(local.x, local.y, atlas_z)),
                0,
            ).x;
        }
        index = (index + 1u) & mask;
    }
    return 0u;
}

fn segment_label_texel(coordinate: vec3i) -> u32 {
    if SEGMENTATION_SPARSE_BRICKS {
        return sparse_label_texel(vec3u(max(coordinate, vec3i(0))));
    }
    return textureLoad(label_texture, coordinate, 0).x;
}
