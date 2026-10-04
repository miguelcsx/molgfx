// Lattice masks use voxel coordinates, so skew and world scaling preserve style.
fn iso_style_visible(coordinate: vec3f) -> bool {
    if VOLUME_RENDER_MODE != VOLUME_RENDER_ISOMESH && VOLUME_RENDER_MODE != VOLUME_RENDER_ISODOTS { return true; }
    let gradient = abs(density_gradient(coordinate));
    var tangent = coordinate.xy;
    if gradient.x >= gradient.y && gradient.x >= gradient.z { tangent = coordinate.yz; }
    else if gradient.y >= gradient.z { tangent = coordinate.xz; }
    let distance = abs(tangent - round(tangent));
    if VOLUME_RENDER_MODE == VOLUME_RENDER_ISOMESH { return min(distance.x, distance.y) <= volume.sampling.w; }
    return dot(distance, distance) <= volume.sampling.w * volume.sampling.w;
}
