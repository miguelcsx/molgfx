use super::*;

/// A straight spine of unit-spaced samples.
fn spine(count: usize) -> Vec<Vec3> {
    let mut points = Vec::with_capacity(count);
    let mut x = 0.0f32;
    for _ in 0..count {
        points.push(Vec3::new(x, 0.0, 0.0));
        x += 1.0;
    }
    points
}

#[test]
fn a_neutral_layer_offsets_nothing() {
    let source = spine(40);
    let mut offsets = Vec::new();
    solve_offsets(&source, SecondaryMotion::default(), &mut offsets);
    assert_eq!(offsets.len(), source.len());
    assert!(
        offsets.iter().all(|offset| *offset == Vec3::ZERO),
        "an amplitude of zero contributes no displacement"
    );
}

#[test]
fn the_anchored_ends_stay_on_the_source_spine() {
    let source = spine(60);
    let mut offsets = Vec::new();
    solve_offsets(&source, SecondaryMotion::gentle(), &mut offsets);
    let Some(first) = offsets.first() else {
        panic!("one offset per sample")
    };
    let Some(last) = offsets.last() else {
        panic!("one offset per sample")
    };
    assert!(first.length() < 1.0e-5, "the first sample is anchored");
    assert!(last.length() < 1.0e-5, "the last sample is anchored");
    assert!(
        offsets.iter().any(|offset| offset.length() > 0.05),
        "the interior actually moves"
    );
}

#[test]
fn the_offset_is_transverse_and_bounded_by_the_amplitude() {
    let source = spine(60);
    let motion = SecondaryMotion {
        amplitude: 1.5,
        ..SecondaryMotion::gentle()
    };
    let mut offsets = Vec::new();
    solve_offsets(&source, motion, &mut offsets);
    for offset in &offsets {
        assert!(
            offset.length() <= motion.amplitude + 1.0e-3,
            "no sample exceeds the requested amplitude"
        );
        // The spine runs along x, so a follow-through must not stretch it.
        assert!(
            offset.x.abs() < 0.35,
            "displacement stays across the spine, got {offset:?}"
        );
    }
}

#[test]
fn the_same_spine_and_phase_resolve_identically() {
    let source = spine(48);
    let motion = SecondaryMotion {
        phase: 0.37,
        ..SecondaryMotion::gentle()
    };
    let mut first = Vec::new();
    let mut second = Vec::new();
    solve_offsets(&source, motion, &mut first);
    solve_offsets(&source, motion, &mut second);
    assert_eq!(first, second, "the solver carries no hidden state");
}

#[test]
fn malformed_settings_resolve_to_bounded_values() {
    let motion = SecondaryMotion {
        amplitude: f32::NAN,
        wavelength_samples: -3.0,
        phase: f32::INFINITY,
        iterations: 0,
        stiffness: 9.0,
        anchor_fraction: -1.0,
    }
    .sanitized();
    assert!(motion.wavelength_samples >= 2.0);
    assert!(motion.phase.is_finite());
    assert!(motion.iterations >= 1);
    assert!((0.0..=1.0).contains(&motion.stiffness));
    assert!((0.0..=0.5).contains(&motion.anchor_fraction));
    assert!(
        motion.is_neutral(),
        "a malformed amplitude contributes nothing"
    );
}
