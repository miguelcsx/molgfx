use crate::{RibbonMesh, RibbonParams, SplineProfile};
use molgfx_math::Vec3;

#[test]
fn zero_radius_traces_and_tubes_keep_live_guides_in_pixel_stroke_quads() {
    let points = [
        Vec3::ZERO,
        Vec3::new(2.0, 1.0, 0.0),
        Vec3::new(4.0, 0.0, 0.0),
    ];
    for (profile, segments) in [(SplineProfile::Trace, 2usize), (SplineProfile::Tube, 12)] {
        let mut mesh = RibbonMesh::default();
        mesh.generate(
            &points,
            &[0, 1, 2],
            RibbonParams {
                profile,
                width: 0.0,
                thickness: 0.0,
                ..RibbonParams::default()
            },
        )
        .unwrap();
        assert_eq!(mesh.vertices.len(), segments * 4);
        assert_eq!(mesh.indices.len(), segments * 6);
        assert!(
            mesh.vertices
                .iter()
                .all(|vertex| Vec3::from_array(vertex.position).is_finite())
        );
        assert_eq!(
            mesh.vertices[0].position.map(f32::to_bits),
            points[0].to_array().map(f32::to_bits)
        );
        assert_eq!(
            mesh.vertices[mesh.vertices.len() - 1]
                .position
                .map(f32::to_bits),
            points[2].to_array().map(f32::to_bits)
        );
    }
}
