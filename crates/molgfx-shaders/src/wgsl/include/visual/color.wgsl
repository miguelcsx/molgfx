// GPU resolution of the per-atom colour schemes.
//
// Colour used to be resolved on the CPU and baked into each 20-byte instance
// record, so changing a scheme repacked and re-uploaded every atom. The record
// now carries the element colour and three palette indices, and this file turns
// them into the final colour under the scheme the representation selects. A
// scheme change is therefore a uniform write and nothing else.

const COLOR_SCHEME_ELEMENT: u32 = 0u;
const COLOR_SCHEME_CHAIN: u32 = 1u;
const COLOR_SCHEME_RESIDUE: u32 = 2u;
const COLOR_SCHEME_SECONDARY: u32 = 3u;
const COLOR_SCHEME_PROPERTY: u32 = 4u;
const COLOR_SCHEME_UNIFORM: u32 = 5u;

/// Where the eight categorical colours begin in the palette.
const COLOR_CATEGORICAL_BASE: u32 = 9u;
/// Where the five secondary-structure colours begin.
const COLOR_SECONDARY_BASE: u32 = 4u;

const COLOR_CHAIN_SHIFT: u32 = 10u;
const COLOR_RESIDUE_SHIFT: u32 = 13u;
const COLOR_SECONDARY_SHIFT: u32 = 16u;
const COLOR_FIELD_MASK: u32 = 7u;

/// The most overriding schemes one overlay table holds.
const COLOR_OVERLAY_CLASSES: u32 = 15u;

/// The colour palette and selector block the representation uploads.
struct ColorUniforms {
    palette: array<vec4f, 17>,
    selector: vec4u,
    appearance: vec4f,
    softness: vec4f,
    /// The selection-scoped overlay: the class column's arena offset (zero
    /// when no overlay applies), its stride in words, and the class count.
    overlay: vec4u,
    /// Two (scheme tag, packed colour) pairs per row, class one first.
    overlay_table: array<vec4u, 8>,
}

@group(2) @binding(18)
var<uniform> color_uniforms: ColorUniforms;

/// One palette slot as a colour.
fn color_palette_slot(slot: u32) -> vec4f {
    return color_uniforms.palette[min(slot, 16u)];
}

/// The three palette indices packed into an atom's semantic word.
fn color_indices(semantic: u32) -> vec3u {
    return vec3u(
        (semantic >> COLOR_CHAIN_SHIFT) & COLOR_FIELD_MASK,
        (semantic >> COLOR_RESIDUE_SHIFT) & COLOR_FIELD_MASK,
        (semantic >> COLOR_SECONDARY_SHIFT) & COLOR_FIELD_MASK,
    );
}

/// Samples the colour property column at one atom row.
///
/// The column's arena offset and stride arrive as uniform words rather than
/// floats, so no conversion is needed before the load. A zero offset means no
/// column was planned, and a NaN sample resolves to the missing colour.
fn color_property_sample(atom_index: u32) -> f32 {
    let offset = color_uniforms.selector.z;
    let stride = color_uniforms.selector.w;
    if offset == 0u {
        return missing_sample();
    }
    return bitcast<f32>(visual_properties[offset + atom_index * stride]);
}

/// The quiet NaN a "no value" sample carries.
///
/// ORed with a masked uniform word rather than written as a bare
/// `bitcast<f32>(0x7fc00000u)`. A constant `NaN` is rejected by the browser's
/// WGSL implementation — "value nan cannot be represented as 'f32'" — while
/// naga accepts it, so the constant form validates at build time and then fails
/// on the first frame of every viewer. The mask folds to zero at run time, so
/// the bits are exactly the quiet NaN this shader means to carry.
fn missing_sample() -> f32 {
    return bitcast<f32>(0x7fc00000u | (color_uniforms.selector.z & 0u));
}

/// Applies a three-stop ramp to a scalar, matching the CPU ramp exactly.
///
/// A non-finite value resolves to the missing colour, which the palette carries
/// packed in the free lane of the ramp row.
fn color_ramp(value: f32) -> vec4f {
    if !(value == value) {
        return unpack4x8unorm(bitcast<u32>(color_uniforms.palette[3].w));
    }
    let low = color_uniforms.palette[3].x;
    let middle = color_uniforms.palette[3].y;
    let high = color_uniforms.palette[3].z;
    var low_color = color_palette_slot(0u);
    var high_color = color_palette_slot(1u);
    var span = middle - low;
    if value > middle {
        low_color = color_palette_slot(1u);
        high_color = color_palette_slot(2u);
        span = high - middle;
    }
    let t = clamp((value - low) / max(span, 1.0e-30), 0.0, 1.0);
    return mix(low_color, high_color, t);
}

/// Whether a scientific appearance mapping drives this representation.
fn color_appearance_enabled() -> bool {
    return color_uniforms.appearance.x < color_uniforms.appearance.y;
}

/// Samples the scientific appearance mapping, returning opacity and softness.
///
/// Opacity and edge softness are scientific data, not decoration, so they are
/// resolved here rather than baked into the shared record: a mapping change then
/// costs one uniform write instead of a repack. A value outside the column, or
/// a non-finite one, resolves to the mapping's own missing response.
fn atom_appearance(semantic: u32, atom_index: u32) -> vec2f {
    if !color_appearance_enabled() {
        // No mapping: full opacity, and the record's packed softness.
        return vec2f(1.0, atom_softness_pixels(semantic));
    }
    let value = color_property_sample(atom_index);
    if !(value == value) {
        return vec2f(color_uniforms.softness.z, color_uniforms.softness.w);
    }
    let domain = color_uniforms.appearance.xy;
    let t = clamp((value - domain.x) / (domain.y - domain.x), 0.0, 1.0);
    let opacity = mix(color_uniforms.appearance.z, color_uniforms.appearance.w, t);
    let softness = mix(color_uniforms.softness.x, color_uniforms.softness.y, t);
    return vec2f(opacity, softness);
}

/// The colour one atom's record carries without the scheme applied.
///
/// A vertex stage writes this: it has the element colour and the material
/// payload, but no need for the property arena, which is not visible to it. The
/// fragment stage finishes the job with [`atom_fragment_color`].
fn atom_record_color(atom: AtomRecord) -> vec4f {
    return atom_color(atom.color);
}

/// Resolves one shaded atom's display colour: scheme, appearance, then style.
///
/// A fragment stage owns this, because a scheme driven by a caller property
/// samples the property arena, which only the fragment stage reads. The record's
/// element colour and the packed semantic indices it forwarded are the inputs.
fn atom_fragment_color(
    element: vec4f,
    semantic: u32,
    entity_id: u32,
) -> vec4f {
    var color = atom_scheme_color(
        semantic,
        atom_source_index(entity_id),
        element,
    );
    color.a *= atom_appearance(semantic, atom_source_index(entity_id)).x;
    color.a *= representation.presentation.x;
    if visual_counts.visual_enabled == 0u {
        return color;
    }
    color = visual_uniform_base_color(color);
    return unpack4x8unorm(visual_result_word(
        atom_source_index(entity_id),
        VISUAL_RESULT_COLOR,
        pack4x8unorm(color),
    ));
}

/// The overlay class of one atom row, or zero when its scheme is not
/// overridden.
///
/// The class column holds whole numbers as scalars; a missing, non-finite or
/// out-of-table value keeps the representation's own scheme.
fn color_overlay_class(atom_index: u32) -> u32 {
    let offset = color_uniforms.overlay.x;
    if offset == 0u {
        return 0u;
    }
    let value = bitcast<f32>(
        visual_properties[offset + atom_index * color_uniforms.overlay.y],
    );
    let count = min(color_uniforms.overlay.z, COLOR_OVERLAY_CLASSES);
    if !(value >= 1.0) || value > f32(count) {
        return 0u;
    }
    return u32(value);
}

/// The (scheme tag, packed colour) pair one non-zero overlay class selects.
fn color_overlay_entry(overlay_class: u32) -> vec2u {
    let slot = overlay_class - 1u;
    let row = color_uniforms.overlay_table[min(slot / 2u, 7u)];
    if (slot & 1u) == 0u {
        return row.xy;
    }
    return row.zw;
}

/// Resolves one atom's display colour under the active scheme.
///
/// `element_color` is the element colour the shared record carries, which every
/// fallback path returns: a scheme whose own input row is absent shows the
/// element colour rather than an arbitrary palette entry.
fn atom_scheme_color(semantic: u32, atom_index: u32, element_color: vec4f) -> vec4f {
    var scheme = color_uniforms.selector.x;
    var packed = color_uniforms.selector.y;
    let overlay_class = color_overlay_class(atom_index);
    if overlay_class != 0u {
        let entry = color_overlay_entry(overlay_class);
        scheme = entry.x;
        packed = entry.y;
    }
    if scheme == COLOR_SCHEME_CHAIN {
        return color_palette_slot(
            COLOR_CATEGORICAL_BASE + color_indices(semantic).x,
        );
    }
    if scheme == COLOR_SCHEME_RESIDUE {
        // Matches the CPU palette, which steps the categorical table by five
        // per residue so adjacent residues are not adjacent colours.
        return color_palette_slot(
            COLOR_CATEGORICAL_BASE + (color_indices(semantic).y * 5u) % 8u,
        );
    }
    if scheme == COLOR_SCHEME_SECONDARY {
        return color_palette_slot(
            COLOR_SECONDARY_BASE + color_indices(semantic).z,
        );
    }
    if scheme == COLOR_SCHEME_PROPERTY {
        return color_ramp(color_property_sample(atom_index));
    }
    if scheme == COLOR_SCHEME_UNIFORM {
        return unpack4x8unorm(packed);
    }
    return element_color;
}
