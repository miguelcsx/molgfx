// Deterministic priority-ordered screen-space label decluttering.
//
// Tile occupancy uses an exact row-major bitset: one bit per 32 px tile.
// The first workgroup clears occupancy in parallel; lane 0 performs the
// priority-sensitive deterministic selection.
//
// Dispatch exactly one workgroup.

//!include "include/camera.wgsl"

const INV_TILE_PIXELS: f32 = 0.03125;
const LABEL_PADDING: f32 = 3.0;

const WORD_SHIFT: u32 = 5u;
const WORD_MASK: u32 = 31u;
const FULL_WORD: u32 = 0xFFFFFFFFu;
const WORKGROUP_SIZE: u32 = 64u;

struct LabelHeaderGpu {
    anchor_width: vec4f,
    bounds: vec4f,
    range: vec4u,
}

struct LabelGpu {
    start_size: vec4f,
    end_offset: vec4f,
    color: vec4f,
    metadata: vec4u,
}

struct DrawIndirectArgs {
    vertex_count: u32,
    instance_count: u32,
    first_vertex: u32,
    first_instance: u32,
}

struct LabelCountsGpu {
    header_count: u32,
    source_count: u32,
    occupancy_count: u32,
    padding: u32,
}

struct ProjectedAnchor {
    pixel: vec2f,
    valid: bool,
}

struct TileRange {
    low: vec2u,
    high: vec2u,
    valid: bool,
}

struct WordRange {
    first: u32,
    last: u32,
    first_mask: u32,
    last_mask: u32,
}

@group(2) @binding(0) var<storage, read> headers: array<LabelHeaderGpu>;
@group(2) @binding(1) var<storage, read> source: array<LabelGpu>;
@group(2) @binding(2) var<storage, read_write> visible: array<LabelGpu>;
@group(2) @binding(3) var<storage, read_write> occupancy: array<u32>;
@group(2) @binding(4) var<storage, read_write> arguments: DrawIndirectArgs;
@group(2) @binding(5) var<uniform> counts: LabelCountsGpu;

/// Projects an anchor into viewport pixels and rejects invalid clip depth.
fn project_anchor(anchor: vec3f) -> ProjectedAnchor {
    let clip = frame.view_proj * vec4f(anchor, 1.0);

    if !(clip.w > 0.0 && clip.z >= 0.0 && clip.z <= clip.w) {
        return ProjectedAnchor(vec2f(0.0), false);
    }

    let ndc = clip.xy * (1.0 / clip.w);

    return ProjectedAnchor(
        (ndc * vec2f(0.5, -0.5) + vec2f(0.5)) * frame.viewport.xy,
        true,
    );
}

/// Returns the viewport tile dimensions.
fn tile_grid_size() -> vec2u {
    return max(
        vec2u(1u),
        vec2u(ceil(frame.viewport.xy * INV_TILE_PIXELS)),
    );
}

/// Converts a projected label rectangle into an inclusive tile range.
fn header_tile_range(
    anchor: vec3f,
    bounds: vec4f,
    limit: vec2u,
) -> TileRange {
    let projected = project_anchor(anchor);

    if !projected.valid {
        return TileRange(vec2u(0u), vec2u(0u), false);
    }

    let padding = vec2f(LABEL_PADDING);
    let low_pixel = projected.pixel + bounds.xy - padding;
    let high_pixel = projected.pixel + bounds.zw + padding;

    if !(
        all(high_pixel >= vec2f(0.0)) &&
        all(low_pixel < frame.viewport.xy)
    ) {
        return TileRange(vec2u(0u), vec2u(0u), false);
    }

    let low = min(
        vec2u(max(low_pixel, vec2f(0.0)) * INV_TILE_PIXELS),
        limit,
    );

    let high = min(
        vec2u(max(high_pixel, vec2f(0.0)) * INV_TILE_PIXELS),
        limit,
    );

    return TileRange(low, high, true);
}

/// Builds the bit masks covering an inclusive horizontal tile range.
fn word_range(low: u32, high: u32) -> WordRange {
    return WordRange(
        low >> WORD_SHIFT,
        high >> WORD_SHIFT,
        FULL_WORD << (low & WORD_MASK),
        FULL_WORD >> (WORD_MASK - (high & WORD_MASK)),
    );
}

/// Tests one bitset row.
fn row_is_free(base: u32, range: WordRange) -> bool {
    if range.first == range.last {
        return (
            occupancy[base + range.first] &
            (range.first_mask & range.last_mask)
        ) == 0u;
    }

    if (occupancy[base + range.first] & range.first_mask) != 0u {
        return false;
    }

    for (
        var word = range.first + 1u;
        word < range.last;
        word++
    ) {
        if occupancy[base + word] != 0u {
            return false;
        }
    }

    return (occupancy[base + range.last] & range.last_mask) == 0u;
}

/// Marks one bitset row occupied.
fn occupy_row(base: u32, range: WordRange) {
    if range.first == range.last {
        occupancy[base + range.first] |=
            range.first_mask & range.last_mask;
        return;
    }

    occupancy[base + range.first] |= range.first_mask;

    for (
        var word = range.first + 1u;
        word < range.last;
        word++
    ) {
        occupancy[base + word] = FULL_WORD;
    }

    occupancy[base + range.last] |= range.last_mask;
}

/// Atomically in algorithmic terms tests and occupies a tile rectangle.
/// Safe without GPU atomics because priority selection runs only on lane 0.
fn try_occupy_range(
    low: vec2u,
    high: vec2u,
    words_per_row: u32,
) -> bool {
    let words = word_range(low.x, high.x);
    let row_count = high.y - low.y + 1u;

    var base = low.y * words_per_row;

    for (var row = 0u; row < row_count; row++) {
        if !row_is_free(base, words) {
            return false;
        }

        base += words_per_row;
    }

    base = low.y * words_per_row;

    for (var row = 0u; row < row_count; row++) {
        occupy_row(base, words);
        base += words_per_row;
    }

    return true;
}

/// Returns the valid source record count for a header range.
fn source_record_count(range: vec4u) -> u32 {
    let first = min(range.x, counts.source_count);

    return min(
        range.y,
        counts.source_count - first,
    );
}

/// Copies an accepted label's records into the compact output.
fn append_visible_records(
    first: u32,
    count: u32,
    destination: u32,
) {
    for (var record = 0u; record < count; record++) {
        visible[destination + record] = source[first + record];
    }
}

@compute @workgroup_size(WORKGROUP_SIZE)
fn declutter_labels(
    @builtin(workgroup_id) workgroup: vec3u,
    @builtin(local_invocation_index) lane: u32,
) {
    // Only one workgroup participates in this deterministic serial pass.
    if any(workgroup != vec3u(0u)) {
        return;
    }

    let grid = tile_grid_size();
    let words_per_row = (grid.x + WORD_MASK) >> WORD_SHIFT;
    let required_words = words_per_row * grid.y;
    let clear_count = min(required_words, counts.occupancy_count);

    for (
        var index = lane;
        index < clear_count;
        index += WORKGROUP_SIZE
    ) {
        occupancy[index] = 0u;
    }

    storageBarrier();

    if lane != 0u {
        return;
    }

    arguments = DrawIndirectArgs(6u, 0u, 0u, 0u);

    // Invalid/undersized occupancy buffer: safely emit nothing.
    if counts.occupancy_count < required_words {
        return;
    }

    let limit = grid - 1u;
    var instance_count = 0u;

    for (
        var header_index = 0u;
        header_index < counts.header_count;
        header_index++
    ) {
        // Load only projection data before knowing whether the label survives.
        let tiles = header_tile_range(
            headers[header_index].anchor_width.xyz,
            headers[header_index].bounds,
            limit,
        );

        if !tiles.valid {
            continue;
        }

        let range = headers[header_index].range;
        let record_count = source_record_count(range);

        // Empty/invalid labels must not reserve screen space.
        if record_count == 0u {
            continue;
        }

        if !try_occupy_range(
            tiles.low,
            tiles.high,
            words_per_row,
        ) {
            continue;
        }

        append_visible_records(
            range.x,
            record_count,
            instance_count,
        );

        instance_count += record_count;
    }

    arguments = DrawIndirectArgs(
        6u,
        instance_count,
        0u,
        0u,
    );
}
