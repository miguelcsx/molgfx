use crate::{RibbonMesh, RibbonParams, SplineProfile};
use molgfx_math::Vec3;

#[test]
fn tube_ends_are_closed_with_outward_normals_and_the_authored_radius() {
    let mut mesh = RibbonMesh::default();
    mesh.generate(
        &[Vec3::ZERO, Vec3::X * 2.0],
        &[1, 2],
        RibbonParams {
            width: 0.6,
            thickness: 0.6,
            profile: SplineProfile::Tube,
            ..RibbonParams::default()
        },
    );
    for (x, sign) in [(0.0, -1.0), (2.0, 1.0)] {
        let triangles: Vec<_> = mesh
            .indices
            .as_chunks::<3>()
            .0
            .iter()
            .filter(|triangle| {
                triangle.iter().all(|&index| {
                    let vertex = mesh.vertices[index as usize];
                    (vertex.position[0] - x).abs() < 1.0e-6
                        && (vertex.normal[0] - sign).abs() < 1.0e-6
                })
            })
            .collect();
        assert!(!triangles.is_empty(), "each end has a visible outward face");
        let area: f32 = triangles
            .iter()
            .map(|triangle| {
                let a = Vec3::from(mesh.vertices[triangle[0] as usize].position);
                let b = Vec3::from(mesh.vertices[triangle[1] as usize].position);
                let c = Vec3::from(mesh.vertices[triangle[2] as usize].position);
                let cross = (b - a).cross(c - a);
                assert!(cross.x * sign > 0.0, "end faces wind outward");
                assert!(
                    [a, b, c]
                        .iter()
                        .all(|point| (point.y * point.y + point.z * point.z).sqrt() <= 0.300_001)
                );
                cross.length() * 0.5
            })
            .sum();
        assert!((area - std::f32::consts::PI * 0.3 * 0.3).abs() < 0.01);
    }
}
