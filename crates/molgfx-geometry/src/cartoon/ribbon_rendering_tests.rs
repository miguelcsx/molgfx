use super::*;

#[test]
fn trace_intervals_stay_straight_and_emit_live_linear_recipes() {
    let points = [Vec3::ZERO, Vec3::X * 3.0, Vec3::new(3.0, 3.0, 0.0)];
    let mut trace = RibbonMesh::default();
    trace
        .generate(
            &points,
            &[0, 1, 2],
            RibbonParams {
                profile: SplineProfile::Trace,
                ..RibbonParams::default()
            },
        )
        .expect("trace generates");
    assert_eq!(trace.samples.len(), 4);
    assert_eq!(trace.samples[1].position, points[1]);
    assert_eq!(trace.samples[2].position, points[1]);
    assert!(
        trace
            .deformations
            .iter()
            .all(|recipe| recipe.parameter[3].to_bits() == 3)
    );
    let mut tube = RibbonMesh::default();
    tube.generate(
        &points,
        &[0, 1, 2],
        RibbonParams {
            profile: SplineProfile::Tube,
            ..RibbonParams::default()
        },
    )
    .expect("tube generates");
    assert!(
        tube.samples
            .iter()
            .any(|sample| sample.position.y < -0.01 || sample.position.x > 3.01)
    );
}

#[test]
fn rocket_helix_cross_sections_share_a_straight_fitted_axis() {
    let points: Vec<_> = (0u16..20)
        .map(|row| {
            let turn = f32::from(row) * 1.7;
            Vec3::new(turn.cos(), turn.sin(), f32::from(row) * 1.5)
        })
        .collect();
    let entities: Vec<_> = (0u32..20).collect();
    let mut mesh = RibbonMesh::default();
    mesh.generate_styled(
        &points,
        &entities,
        &[SecondaryStructure::AlphaHelix; 20],
        RibbonParams {
            profile: SplineProfile::Rocket,
            ..RibbonParams::default()
        },
    )
    .expect("rocket generates");
    let first = mesh.samples.first().expect("first sample").position;
    let last = mesh.samples.last().expect("last sample").position;
    let axis = (last - first).normalize();
    assert!(
        mesh.samples
            .iter()
            .all(|sample| (sample.position - first).cross(axis).length() < 1e-4)
    );
    assert!(
        mesh.deformations
            .iter()
            .all(|recipe| recipe.parameter[3].to_bits() == 6)
    );
}

#[test]
fn helix_fit_jobs_visit_each_guide_once_and_repeated_preparation_reuses_storage() {
    let points = [Vec3::ZERO, Vec3::Y, Vec3::Y * 2.0, Vec3::Y * 3.0];
    let mut mesh = RibbonMesh::default();
    mesh.generate_styled(
        &points,
        &[0, 1, 2, 3],
        &[SecondaryStructure::AlphaHelix; 4],
        RibbonParams {
            profile: SplineProfile::Rocket,
            ..RibbonParams::default()
        },
    )
    .expect("rocket");
    mesh.prepare_helix_fits().expect("jobs");
    let jobs = mesh.helix_fit_jobs().to_vec();
    assert_eq!(jobs[0], 1);
    assert_eq!(jobs[2], 4);
    assert_eq!(&jobs[jobs[1] as usize..], &[0, 1, 2, 3]);
    let lanes = mesh.radius_source_values().len();
    mesh.prepare_helix_fits().expect("reused jobs");
    assert_eq!(mesh.helix_fit_jobs(), jobs);
    assert_eq!(mesh.radius_source_values().len(), lanes);
    mesh.clear();
    assert!(mesh.helix_fit_jobs().is_empty());
}
