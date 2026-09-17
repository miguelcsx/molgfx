use super::*;
use molgfx_core::{ClipCap, ClipPlane};
use molgfx_math::Rgba8;

fn vertex(position: [f32; 3]) -> RibbonVertex {
    RibbonVertex {
        position,
        entity_id: 0,
        normal: [0.0, 0.0, 1.0],
        color: Rgba8::WHITE,
    }
}

#[test]
fn a_solid_plane_adds_a_closed_cross_section() {
    let mut vertices = vec![
        vertex([-1.0, -1.0, -1.0]),
        vertex([-1.0, 1.0, -1.0]),
        vertex([1.0, 1.0, -1.0]),
        vertex([1.0, -1.0, -1.0]),
        vertex([-1.0, -1.0, 1.0]),
        vertex([-1.0, 1.0, 1.0]),
        vertex([1.0, 1.0, 1.0]),
        vertex([1.0, -1.0, 1.0]),
    ];
    let mut indices = vec![
        0, 1, 2, 0, 2, 3, 4, 6, 5, 4, 7, 6, 0, 4, 5, 0, 5, 1, 1, 5, 6, 1, 6, 2, 2, 6, 7, 2, 7, 3,
        3, 7, 4, 3, 4, 0,
    ];
    let Ok(plane) = ClipPlane::from_point_normal(Vec3::ZERO, Vec3::Z) else {
        panic!("plane validates")
    };
    let Ok(clipping) = ClipSet::new(&[plane]).map(|set| set.with_cap(ClipCap::Solid)) else {
        panic!("clip set validates")
    };

    append_caps(
        &mut vertices,
        &mut indices,
        0..8,
        0..36,
        Mat4::IDENTITY,
        clipping,
    );

    assert!(vertices.len() > 8);
    assert!(indices.len() > 36);
    assert!(
        vertices[8..]
            .iter()
            .all(|cap| cap.position[2].abs() <= EPSILON)
    );
}

#[test]
fn an_open_clip_does_not_add_geometry() {
    let mut vertices = vec![
        vertex([-1.0, 0.0, -1.0]),
        vertex([1.0, 0.0, 1.0]),
        vertex([0.0, 1.0, 1.0]),
    ];
    let mut indices = vec![0, 1, 2];
    let Ok(plane) = ClipPlane::from_point_normal(Vec3::ZERO, Vec3::Z) else {
        panic!("plane validates")
    };
    let Ok(clipping) = ClipSet::new(&[plane]).map(|set| set.with_cap(ClipCap::Open)) else {
        panic!("clip set validates")
    };

    append_caps(
        &mut vertices,
        &mut indices,
        0..3,
        0..3,
        Mat4::IDENTITY,
        clipping,
    );

    assert_eq!(vertices.len(), 3);
    assert_eq!(indices.len(), 3);
}
