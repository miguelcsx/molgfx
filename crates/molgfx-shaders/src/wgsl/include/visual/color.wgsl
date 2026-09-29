// GPU resolution of the per-atom colour schemes.
//
// Colour used to be resolved on the CPU and baked into each instance record, so
// changing a scheme repacked and re-uploaded every atom. The record now carries
// the element colour only, and this file turns it into the final colour under
// the rule the representation selects. A scheme change is therefore a uniform
// write and nothing else.
//
// Every scheme is a rule: a tag, an optional packed colour, and the arena
// column it reads. The representation's base scheme is one rule and each entry
// of a selection-scoped overlay is another; one function resolves both.

const COLOR_RULE_ELEMENT: u32 = 0u;
const COLOR_RULE_UNIFORM: u32 = 1u;
const COLOR_RULE_PROPERTY: u32 = 2u;
const COLOR_RULE_CATEGORY: u32 = 3u;

/// The most overriding schemes one overlay table holds.
const COLOR_OVERLAY_CLASSES: u32 = 15u;
/// Colours in the palette bank.
const COLOR_BANK_COLORS: u32 = 128u;

//!include "include/color/ramp.wgsl"

/// One colour rule.
struct ColorRule {
    /// x = tag, y = packed uniform colour, z = column offset, w = column stride.
    header: vec4u,
    /// x = palette bank base, y = palette length.
    palette: vec4u,
}

/// The colour block the representation uploads.
struct ColorUniforms {
    base: ColorRule,
    appearance: vec4f,
    softness: vec4f,
    /// The appearance column: x = arena offset, y = stride.
    appearance_column: vec4u,
    /// The selection-scoped overlay: the class column's arena offset (zero
    /// when no overlay applies), its stride in words, and the class count.
    overlay: vec4u,
    overlay_rules: array<ColorRule, 15>,
    /// Every built-in palette, four packed colours per element.
    bank: array<vec4u, 32>,
    /// The property ramp, baked to a lookup table.
    ramp: RampLut,
}

@group(2) @binding(18)
var<uniform> color_uniforms: ColorUniforms;

/// Samples one arena column at one atom row.
///
/// The offset and stride arrive as uniform words rather than floats, so no
/// conversion is needed before the load. A zero offset means no column was
/// planned, and the sample is the quiet NaN of a missing value.
fn color_column_sample(offset: u32, stride: u32, atom_index: u32) -> f32 {
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
    return bitcast<f32>(0x7fc00000u | (color_uniforms.base.header.z & 0u));
}

/// Resolves a scalar through the property ramp table, matching the CPU ramp.
///
/// A non-finite value resolves to the missing colour the table carries.
fn color_ramp(value: f32) -> vec4f {
    let ramp = color_uniforms.ramp;
    if !(value == value) {
        return unpack4x8unorm(bitcast<u32>(ramp.domain.z));
    }
    let tap = ramp_tap(value, ramp.domain.x, ramp.domain.y);
    return ramp_mix(
        ramp.colors[tap.low >> 2u][tap.low & 3u],
        ramp.colors[tap.high >> 2u][tap.high & 3u],
        tap.fraction,
    );
}

/// Whether a physical appearance mapping drives this representation.
fn color_appearance_enabled() -> bool {
    return color_uniforms.appearance.x < color_uniforms.appearance.y;
}

/// Samples the physical appearance mapping, returning opacity and softness.
///
/// Opacity and edge softness are physical data, not decoration, so they are
/// resolved here rather than baked into the shared record: a mapping change then
/// costs one uniform write instead of a repack. A value outside the column, or
/// a non-finite one, resolves to the mapping's own missing response.
fn atom_appearance(semantic: u32, atom_index: u32) -> vec2f {
    if !color_appearance_enabled() {
        // No mapping: full opacity, and the record's packed softness.
        return vec2f(1.0, atom_softness_pixels(semantic));
    }
    let value = color_column_sample(
        color_uniforms.appearance_column.x,
        color_uniforms.appearance_column.y,
        atom_index,
    );
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
    let source = atom_source_index(entity_id);
    color.a *= atom_appearance(semantic, source).x;
    color.a *= representation.presentation.x;
    if visual_counts.visual_enabled != 0u {
        color = visual_uniform_base_color(color);
        color = unpack4x8unorm(visual_result_word(
            source,
            VISUAL_RESULT_COLOR,
            pack4x8unorm(color),
        ));
    }
    return interaction_color(color, source);
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

//!include "include/visual/interaction.wgsl"

/// The colour a categorical rule gives one atom.
///
/// The category is a whole-number scalar reduced modulo the palette length. An
/// atom with no category, or a negative or non-finite one, keeps its element
/// colour, which is how a scheme can colour only some atoms.
fn color_category(rule: ColorRule, atom_index: u32, element_color: vec4f) -> vec4f {
    let value = color_column_sample(rule.header.z, rule.header.w, atom_index);
    if !(value >= 0.0) || value > 16777216.0 {
        return element_color;
    }
    let index = rule.palette.x + u32(value) % max(rule.palette.y, 1u);
    let slot = min(index, COLOR_BANK_COLORS - 1u);
    return unpack4x8unorm(color_uniforms.bank[slot >> 2u][slot & 3u]);
}

/// Applies one rule to one atom.
///
/// `element_color` is the element colour the shared record carries, which every
/// fallback path returns: a rule whose own input is absent shows the element
/// colour rather than an arbitrary palette entry.
fn color_rule_apply(rule: ColorRule, atom_index: u32, element_color: vec4f) -> vec4f {
    let tag = rule.header.x;
    if tag == COLOR_RULE_CATEGORY {
        return color_category(rule, atom_index, element_color);
    }
    if tag == COLOR_RULE_PROPERTY {
        return color_ramp(color_column_sample(rule.header.z, rule.header.w, atom_index));
    }
    if tag == COLOR_RULE_UNIFORM {
        return unpack4x8unorm(rule.header.y);
    }
    return element_color;
}

/// Resolves one atom's display colour under the active scheme.
fn atom_scheme_color(semantic: u32, atom_index: u32, element_color: vec4f) -> vec4f {
    let overlay_class = color_overlay_class(atom_index);
    if overlay_class != 0u {
        let slot = min(overlay_class, COLOR_OVERLAY_CLASSES) - 1u;
        return color_rule_apply(color_uniforms.overlay_rules[slot], atom_index, element_color);
    }
    return color_rule_apply(color_uniforms.base, atom_index, element_color);
}
