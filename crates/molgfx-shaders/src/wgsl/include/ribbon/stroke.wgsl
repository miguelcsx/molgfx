// Polymer lines use live guide coordinates and expand only in screen space.

fn ribbon_stroke_mode(mode: u32) -> bool { return mode == 12u || mode == 13u; }

fn ribbon_stroke_point(recipe: RibbonDeformation, t: f32, previous: bool) -> vec3f {
    let a = ribbon_coordinate(recipe.controls.y, previous);
    let b = ribbon_coordinate(recipe.controls.z, previous);
    if bitcast<u32>(recipe.parameter.w) == 12u { return mix(a, b, t); }
    return ribbon_catmull_position(ribbon_coordinate(recipe.controls.x, previous),
        a, b, ribbon_coordinate(recipe.controls.w, previous), t);
}

fn ribbon_stroke_offset(vertex_id: u32, clip: vec4f) -> vec4f {
    let recipe = ribbon_deformations[vertex_id];
    if !ribbon_stroke_mode(bitcast<u32>(recipe.parameter.w)) { return clip; }
    let neighbor = ribbon_stroke_point(recipe, recipe.parameter.z, false);
    let other = frame.view_proj * model.model_to_world * vec4f(neighbor, 1.0);
    if clip.w <= 1e-6 || other.w <= 1e-6 { return clip; }
    var delta = (other.xy / other.w - clip.xy / clip.w) * frame.viewport.xy;
    // Endpoint-local neighbor vectors point in opposite directions.
    if recipe.parameter.x > recipe.parameter.z { delta = -delta; }
    if dot(delta, delta) <= 1e-12 { return clip; }
    let side = normalize(vec2f(-delta.y, delta.x));
    let offset = side * recipe.parameter.y * ribbon_uniforms.tube.y * frame.viewport.zw * clip.w;
    return vec4f(clip.xy + offset, clip.zw);
}
