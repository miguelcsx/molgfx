use super::*;

fn hexagon(z: [f32; 6]) -> Vec<Vec3> {
    (0..6u8)
        .map(|index| {
            let angle = std::f32::consts::TAU * f32::from(index) / 6.0;
            Vec3::new(angle.cos(), angle.sin(), z[usize::from(index)])
        })
        .collect()
}

#[test]
fn one_ring_emits_a_closed_faceted_plate() {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    emit_plate(
        &hexagon([0.0; 6]),
        0.25,
        255,
        7,
        &mut vertices,
        &mut indices,
    );
    assert_eq!(
        vertices.len(),
        72,
        "two caps and two rim triangles per edge"
    );
    assert_eq!(indices.len(), 72, "every emitted vertex is indexed once");
    assert!(
        indices
            .iter()
            .all(|index| usize::try_from(*index).is_ok_and(|i| i < vertices.len()))
    );
}

#[test]
fn puckering_moves_colour_from_planar_red_toward_cold_hues() {
    let planar = hexagon([0.0; 6]);
    let chair = hexagon([0.5, -0.5, 0.5, -0.5, 0.5, -0.5]);
    let origin = Vec3::ZERO;
    let planar_color = pucker_color(pucker_amplitude(&planar, origin, Vec3::Z), 255);
    let chair_color = pucker_color(pucker_amplitude(&chair, origin, Vec3::Z), 255);
    assert!(planar_color.r > planar_color.g);
    assert!(chair_color.b > planar_color.b || chair_color.g > planar_color.g);
}

#[test]
fn a_plate_is_thin_enough_to_read_as_a_sheet() {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    emit_plate(
        &hexagon([0.0; 6]),
        0.16,
        255,
        7,
        &mut vertices,
        &mut indices,
    );
    let depth = vertices
        .iter()
        .map(|vertex| vertex.position[2])
        .fold(f32::NEG_INFINITY, f32::max);
    assert!(
        depth < 0.2,
        "half-depth stays well under the 1.0 ring radius"
    );
    assert!(
        vertices.iter().any(|vertex| vertex.normal[2].abs() < 0.1),
        "the rim carries an in-plane normal, so the plate has a lit edge"
    );
}
