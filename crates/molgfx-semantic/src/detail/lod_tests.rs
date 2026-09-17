use super::*;

#[test]
fn projected_error_walks_the_biological_ladder() {
    let policy = LodPolicy {
        hysteresis: 0.0,
        ..LodPolicy::default()
    };
    assert_eq!(policy.select(8.0, 1.0, LodLevel::Atom), LodLevel::Atom);
    assert_eq!(policy.select(2.0, 1.0, LodLevel::Atom), LodLevel::Residue);
    assert_eq!(
        policy.select(0.5, 1.0, LodLevel::Residue),
        LodLevel::SecondaryStructure
    );
    assert_eq!(policy.select(0.1, 1.0, LodLevel::Domain), LodLevel::Domain);
}

#[test]
fn hysteresis_prevents_boundary_flapping() {
    let policy = LodPolicy::default();
    assert_eq!(policy.select(3.9, 1.0, LodLevel::Atom), LodLevel::Atom);
    assert_eq!(
        policy.select(4.1, 1.0, LodLevel::Residue),
        LodLevel::Residue
    );
}
