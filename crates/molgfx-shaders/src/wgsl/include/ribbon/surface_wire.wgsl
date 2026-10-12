// Pixel distances to the provider's actual triangle edges, shared by all passes.
fn ribbon_surface_wire_clipped(barycentric: vec3f) -> bool {
    let horizontal = dpdx(barycentric);
    let vertical = dpdy(barycentric);
    let gradient = max(sqrt(horizontal * horizontal + vertical * vertical), vec3f(1e-10));
    let distance = barycentric / gradient;
    return ribbon_uniforms.tube.z != 0.0 && min(min(distance.x, distance.y), distance.z) > ribbon_uniforms.tube.y * 0.5;
}
