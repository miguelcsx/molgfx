// Each ring guide gathers the complete bonded cycle, including internal motion.
struct TwisterGuide { center: vec3f, normal: vec3f, }
struct TwisterFrame { basis: mat3x3f, valid: bool, }

fn twister_atom(row: u32, previous: bool, reference: bool) -> vec3f {
    if reference { return ribbon_base_coordinate(row); }
    return ribbon_coordinate(row, previous);
}

fn twister_guide(offset: u32, previous: bool, reference: bool) -> TwisterGuide {
    let count = bitcast<u32>(ribbon_radius_sources[offset]);
    var center = vec3f(0.0);
    for (var index = 0u; index < count; index += 1u) {
        let row = bitcast<u32>(ribbon_radius_sources[offset + 1u + index]);
        center += twister_atom(row, previous, reference);
    }
    center /= f32(count);
    var normal = vec3f(0.0);
    for (var index = 0u; index < count; index += 1u) {
        let row = bitcast<u32>(ribbon_radius_sources[offset + 1u + index]);
        let next = bitcast<u32>(ribbon_radius_sources[offset + 1u + (index + 1u) % count]);
        normal += cross(twister_atom(row, previous, reference) - center,
            twister_atom(next, previous, reference) - center);
    }
    if dot(normal, normal) > 1e-12 { normal = normalize(normal); }
    return TwisterGuide(center, normal);
}

fn twister_frame(tangent: vec3f, left: vec3f, right: vec3f, amount: f32) -> TwisterFrame {
    let first = left - tangent * dot(left, tangent);
    if dot(first, first) < 1e-8 {
        return TwisterFrame(mat3x3f(), false);
    }
    let start = normalize(first);
    var end = right - tangent * dot(right, tangent);
    if dot(end, end) < 1e-8 { end = start; } else { end = normalize(end); }
    let across = cross(tangent, start);
    let angle = atan2(dot(end, across), dot(end, start));
    let eased = amount * amount * (3.0 - 2.0 * amount);
    let turned = angle * eased;
    let binormal = normalize(start * cos(turned) + across * sin(turned));
    let normal = normalize(cross(binormal, tangent));
    return TwisterFrame(mat3x3f(tangent, normal, binormal), true);
}

fn twister_vertex(deformation: RibbonDeformation, base_position: vec3f,
    base_normal: vec3f, previous: bool) -> DeformedRibbonVertex {
    let p0 = twister_guide(deformation.controls.x, previous, false);
    let p1 = twister_guide(deformation.controls.y, previous, false);
    let p2 = twister_guide(deformation.controls.z, previous, false);
    let p3 = twister_guide(deformation.controls.w, previous, false);
    let r0 = twister_guide(deformation.controls.x, false, true);
    let r1 = twister_guide(deformation.controls.y, false, true);
    let r2 = twister_guide(deformation.controls.z, false, true);
    let r3 = twister_guide(deformation.controls.w, false, true);
    let t = deformation.parameter.x;
    let center = ribbon_catmull_position(p0.center, p1.center, p2.center, p3.center, t);
    let reference = ribbon_catmull_position(r0.center, r1.center, r2.center, r3.center, t);
    let derivative = ribbon_catmull_tangent(p0.center, p1.center, p2.center, p3.center, t);
    let reference_derivative = ribbon_catmull_tangent(r0.center, r1.center, r2.center, r3.center, t);
    if dot(derivative, derivative) < 1e-12 || dot(reference_derivative, reference_derivative) < 1e-12 {
        return DeformedRibbonVertex(center, base_normal, 1u);
    }
    let tangent = normalize(derivative);
    let reference_tangent = normalize(reference_derivative);
    let frame = twister_frame(tangent, p1.normal, p2.normal, t);
    let base_frame = twister_frame(reference_tangent, r1.normal, r2.normal, t);
    if frame.valid && base_frame.valid {
        let rotation = frame.basis * transpose(base_frame.basis);
        return DeformedRibbonVertex(center + rotation * (base_position - reference),
            normalize(rotation * base_normal), 1u);
    }
    let rotation = ribbon_rotation_arc(reference_tangent, tangent);
    return DeformedRibbonVertex(center + ribbon_rotate(rotation, base_position - reference),
        normalize(ribbon_rotate(rotation, base_normal)), 1u);
}
