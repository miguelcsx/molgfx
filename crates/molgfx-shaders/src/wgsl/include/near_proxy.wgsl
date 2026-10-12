// Conservative screen bounds of a view-space box clipped to the near plane.
// Only near-crossing impostors pay this cost; offscreen boxes collapse instead
// of running an analytic intersection over the whole viewport.
fn near_proxy_ndc(minimum: vec3f, maximum: vec3f, corner: vec2f) -> vec2f {
    let near_z = camera_near_z();
    if minimum.z >= near_z {
        return vec2f(0.0);
    }
    let front_z = min(maximum.z, near_z);
    var lower = vec2f(1.0);
    var upper = vec2f(-1.0);
    for (var index = 0u; index < 8u; index += 1u) {
        let point = vec3f(
            select(minimum.x, maximum.x, (index & 1u) != 0u),
            select(minimum.y, maximum.y, (index & 2u) != 0u),
            select(minimum.z, front_z, (index & 4u) != 0u),
        );
        let clip = camera_view_clip(point);
        let ndc = clip.xy / clip.w;
        lower = min(lower, ndc);
        upper = max(upper, ndc);
    }
    return mix(clamp(lower, vec2f(-1.0), vec2f(1.0)),
        clamp(upper, vec2f(-1.0), vec2f(1.0)), corner);
}
