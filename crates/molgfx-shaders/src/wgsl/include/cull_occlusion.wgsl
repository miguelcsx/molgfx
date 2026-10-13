// Occlusion culling for opaque sphere sets above the level-of-detail threshold
// (cull mode 3), so the drawn instance count follows what is visible rather
// than what is in the frustum.
//
// Two screen-space depth planes live in `tile_depth`: one texel per camera
// pixel followed by one per light-view shadow texel. Every frustum-visible
// atom first records the depth of its far side in the plane cell its centre
// falls in (atomicMin over the float bits, valid for non-negative floats). An
// atom is then dropped only when, at its centre and at four points a footprint
// radius away, a nearer atom already lies wholly in front of its nearest
// surface in both planes. Atoms whose footprint reaches outside a plane stay.
//
// Sub-pixel atoms stand in for their cell, which is the fallback that keeps the
// result O(visible) when atoms are smaller than a pixel: the nearest atom of a
// cell stays, those behind it by more than their own depth are dropped. A
// footprint above `OCCLUSION_MAX_PIXELS` is never tested, so the few large
// atoms are always drawn exactly.

const SHADOW_TEXELS: u32 = 2048u;
const OCCLUSION_MAX_PIXELS: f32 = 16.0;
// Accumulated samples shift the image by up to this many pixels after the cull
// ran on the first one, so every footprint is widened by it.
const OCCLUSION_JITTER_PIXELS: f32 = 1.0;
const OCCLUDER_NONE: u32 = 0xffffffffu;

struct AtomBody {
    center: vec3f,
    // Non-positive: the atom draws nothing.
    radius: f32,
}

/// One atom's world-space centre and drawn radius.
fn atom_body(index: u32) -> AtomBody {
    let atom = input_atoms[index];
    let molecular = bead_geometry(atom, false);
    var radius = molecular.w;
    if (atom.element_flags & VISIBLE_FLAG) == 0u
        || (atom.element_flags & HIDE_MARKER_FLAG) != 0u || radius <= 0.0 {
        return AtomBody(vec3f(0.0), 0.0);
    }
    let geometry = visual_result_word(
        atom.entity_id & ENTITY_ID_MASK,
        VISUAL_RESULT_GEOMETRY,
        pack4x8unorm(visual_config.uniform_geometry),
    );
    if counts.visual_enabled != 0u && (geometry & 0xffu) == 0u {
        return AtomBody(vec3f(0.0), 0.0);
    }
    if counts.visual_enabled != 0u {
        radius *= f32((geometry >> 16u) & 0xffu) / 64.0;
    }
    var local = molecular.xyz;
    if counts.visual_enabled != 0u { local += visual_result_offset(atom.entity_id & ENTITY_ID_MASK); }
    let world =
        model_to_world[0].xyz * local.x
        + model_to_world[1].xyz * local.y
        + model_to_world[2].xyz * local.z
        + model_to_world[3].xyz;
    return AtomBody(world, radius);
}

/// 0: outside the frustum; 1: crossing the near plane, always drawn; 2: inside.
fn frustum_state(body: AtomBody) -> u32 {
    if body.radius <= 0.0 {
        return 0u;
    }
    let near_state = near_visibility(body.center, body.radius);
    if near_state == 0u {
        return 0u;
    }
    if near_state == 1u {
        return 1u;
    }
    let clip = frame.view_proj * vec4f(body.center, 1.0);
    let w = clip.w;
    if w <= 0.0 || clip.z < 0.0 || clip.z > w {
        return 0u;
    }
    let extent = frame.projection_kind.yz * body.radius;
    if !all(abs(clip.xy) <= vec2f(w, w) + extent) {
        return 0u;
    }
    return 2u;
}

fn camera_plane_size() -> vec2u {
    return vec2u(u32(frame.viewport.x), u32(frame.viewport.y));
}

fn occlusion_words() -> u32 {
    let size = camera_plane_size();
    return size.x * size.y + SHADOW_TEXELS * SHADOW_TEXELS;
}

/// Mode 3 needs the whole depth plane; a smaller buffer degrades to no culling.
fn occlusion_ready() -> bool {
    return counts.lod_enabled == 3u && arrayLength(&tile_depth) >= occlusion_words();
}

/// One plane's view of an atom: the cell it centres on, the footprint radius in
/// cells, and the depths of its near and far sides.
struct PlaneAtom {
    cell: vec2f,
    footprint: f32,
    near: f32,
    far: f32,
    valid: bool,
}

fn camera_plane_atom(body: AtomBody) -> PlaneAtom {
    let view_z = dot(vec3f(frame.view[0].z, frame.view[1].z, frame.view[2].z), body.center)
        + frame.view[3].z;
    let clip = frame.view_proj * vec4f(body.center, 1.0);
    let extent = frame.projection_kind.yz * body.radius;
    let ndc = clip.xy / clip.w;
    let cell = (ndc * vec2f(0.5, -0.5) + vec2f(0.5)) * frame.viewport.xy;
    let footprint = max(
        extent.x / clip.w * frame.viewport.x * 0.5,
        extent.y / clip.w * frame.viewport.y * 0.5,
    );
    return PlaneAtom(cell, footprint, max(-view_z - body.radius, 0.0),
        max(-view_z + body.radius, 0.0), true);
}

fn light_plane_atom(body: AtomBody) -> PlaneAtom {
    if frame.shadow_projection[0].x == 0.0 {
        return PlaneAtom(vec2f(0.0), 0.0, 0.0, 0.0, false);
    }
    let light = frame.shadow_view[0].xyz * body.center.x
        + frame.shadow_view[1].xyz * body.center.y
        + frame.shadow_view[2].xyz * body.center.z
        + frame.shadow_view[3].xyz;
    let clip = frame.shadow_projection * vec4f(light, 1.0);
    let cell = (clip.xy * 0.5 + vec2f(0.5)) * f32(SHADOW_TEXELS);
    let footprint = body.radius * 0.5 * f32(SHADOW_TEXELS)
        * max(abs(frame.shadow_projection[0].x), abs(frame.shadow_projection[1].y));
    return PlaneAtom(cell, footprint, max(-light.z - body.radius, 0.0),
        max(-light.z + body.radius, 0.0), true);
}

fn plane_index(base: u32, size: vec2u, cell: vec2f) -> u32 {
    let at = vec2u(cell);
    return base + at.y * size.x + at.x;
}

fn inside_plane(size: vec2u, cell: vec2f) -> bool {
    return cell.x >= 0.0 && cell.y >= 0.0 && cell.x < f32(size.x) && cell.y < f32(size.y);
}

fn record_plane(base: u32, size: vec2u, atom: PlaneAtom) {
    if atom.valid && inside_plane(size, atom.cell) {
        atomicMin(&tile_depth[plane_index(base, size, atom.cell)], bitcast<u32>(atom.far));
    }
}

/// Whether a nearer atom hides this one at the cell, with the cell outside the
/// plane counting as not hidden.
fn hidden_at(base: u32, size: vec2u, cell: vec2f, near: f32) -> bool {
    if !inside_plane(size, cell) {
        return false;
    }
    return atomicLoad(&tile_depth[plane_index(base, size, cell)]) < bitcast<u32>(near);
}

fn plane_hides(base: u32, size: vec2u, atom: PlaneAtom) -> bool {
    if !atom.valid {
        return false;
    }
    if atom.footprint > OCCLUSION_MAX_PIXELS {
        return false;
    }
    let reach = atom.footprint + OCCLUSION_JITTER_PIXELS;
    return hidden_at(base, size, atom.cell, atom.near)
        && hidden_at(base, size, atom.cell + vec2f(reach, 0.0), atom.near)
        && hidden_at(base, size, atom.cell - vec2f(reach, 0.0), atom.near)
        && hidden_at(base, size, atom.cell + vec2f(0.0, reach), atom.near)
        && hidden_at(base, size, atom.cell - vec2f(0.0, reach), atom.near);
}

/// Clears both planes; every invocation strides over its share of the words.
fn occlusion_reset(first: u32, stride: u32) {
    if !occlusion_ready() {
        return;
    }
    let words = occlusion_words();
    for (var word = first; word < words; word += stride) {
        atomicStore(&tile_depth[word], OCCLUDER_NONE);
    }
}

/// Pass one: records the far side of every frustum-visible atom in both planes.
fn occlusion_record(index: u32) {
    if !occlusion_ready() {
        return;
    }
    let body = atom_body(index);
    if frustum_state(body) != 2u {
        return;
    }
    let camera_size = camera_plane_size();
    record_plane(0u, camera_size, camera_plane_atom(body));
    record_plane(camera_size.x * camera_size.y, vec2u(SHADOW_TEXELS), light_plane_atom(body));
}

/// Pass two: whether this atom is drawn, to be called after every atom has
/// recorded. It is drawn when it is in the frustum and either plane still
/// shows it.
fn occlusion_keep(index: u32) -> bool {
    let body = atom_body(index);
    let state = frustum_state(body);
    if state != 2u || !occlusion_ready() {
        return state != 0u;
    }
    let camera_size = camera_plane_size();
    if !plane_hides(0u, camera_size, camera_plane_atom(body)) {
        return true;
    }
    // Without a light plane nothing is cast, so the camera verdict stands.
    let light = light_plane_atom(body);
    return light.valid
        && !plane_hides(camera_size.x * camera_size.y, vec2u(SHADOW_TEXELS), light);
}
